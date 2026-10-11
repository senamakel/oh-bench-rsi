//! `ohrsi-swarm` — runs the memory-rsi hive: one OpenHuman agent per seat of
//! a tinyhivemind-language package, on a durable tinyhivemind coordinator.
//!
//! ```text
//! ohrsi-swarm validate [hive-dir]
//! ohrsi-swarm run --hive hives/memory-rsi --run-dir runs/<run> --brief <file>
//!                 --gateway http://127.0.0.1:8790 [--tinymemory <repo>]
//!                 [--wall-secs 3600] [--turn-secs 1200]
//!                 [--run-id <id>] [--ledger-dir runs/<run>]
//! ```
//!
//! Every model call goes through the spend gateway, tagged
//! `/__tag/swarm/<seat>/`, so the run's cap covers the swarm and the meter
//! attributes spend per seat. Editing seats get their own TinyMemory worktree
//! (see `worktree.rs`); read-only seats see the incumbent. The live trace is
//! `<run-dir>/swarm/events.jsonl` (see `trace.rs`).

mod hooks;
mod package;
mod trace;
mod worktree;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, bail};
use openhuman_embed::{
    Access, AgentDefinitionSpec, AgentSpec, GroupMode, Provider, Runtime, ToolGroups, ToolScopeSpec, Workspace,
};
use serde_json::{Value, json};
use tinyhivemind_hives::{Coordinator, CoordinatorOptions, Destination, HiveInfo, SendMessage, SqliteStorage};
use tinyhivemind_openhuman::OpenHumanHost;

use crate::trace::{Trace, clip};

/// The permanent tools `tinyhivemind-openhuman` attaches to every seat.
const HIVEMIND_TOOLS: &[&str] = &[
    "hivemind_ask",
    "hivemind_broadcast",
    "hivemind_complete",
    "hivemind_list_agents",
    "hivemind_list_hives",
    "hivemind_post",
    "hivemind_read",
    "hivemind_send_agent",
    "hivemind_send_hive",
];

struct Options {
    hive: PathBuf,
    run_dir: PathBuf,
    brief: PathBuf,
    gateway: String,
    tinymemory: PathBuf,
    wall: Duration,
    turn: Duration,
    run_id: Option<String>,
    ledger_dir: Option<PathBuf>,
}

fn parse(args: &[String]) -> anyhow::Result<Options> {
    let get = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let secs = |name: &str, default: u64| get(name).map_or(Ok(default), |v| v.parse()).map(Duration::from_secs);
    Ok(Options {
        hive: get("--hive").unwrap_or_else(|| "hives/memory-rsi".into()).into(),
        run_dir: get("--run-dir").context("--run-dir is required")?.into(),
        brief: get("--brief").context("--brief is required")?.into(),
        gateway: get("--gateway").unwrap_or_else(|| "http://127.0.0.1:8790".into()),
        tinymemory: get("--tinymemory").unwrap_or_else(|| "vendor/openhuman/vendor/tinymemory".into()).into(),
        wall: secs("--wall-secs", 3600)?,
        turn: secs("--turn-secs", 1200)?,
        run_id: get("--run-id"),
        ledger_dir: get("--ledger-dir").map(PathBuf::from),
    })
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("validate") => {
            let dir = args.get(1).map_or("hives/memory-rsi", String::as_str);
            let lowered = package::load(Path::new(dir))?;
            println!(
                "{}: {} seats ({}), {} desks",
                lowered.id,
                lowered.seats.len(),
                lowered.seats.iter().map(|s| s.id.as_str()).collect::<Vec<_>>().join(", "),
                lowered.desks.len()
            );
            Ok(())
        }
        Some("run") => {
            let options = parse(&args[1..])?;
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_stack_size(16 * 1024 * 1024)
                .build()?;
            runtime.block_on(async { tokio::spawn(run(options)).await })?
        }
        _ => bail!("usage: ohrsi-swarm validate [hive-dir] | run --run-dir <dir> --brief <file> [...]"),
    }
}

/// The text each agent is built with: seat prompt, role charter, shared context.
fn system_prompt(hive_dir: &Path, seat: &tinyhivemind_lang::LoweredSeat, role: &str, context: &BTreeMap<String, String>, action_dir: &Path) -> anyhow::Result<String> {
    let charter = std::fs::read_to_string(hive_dir.join(format!("roles/{role}.md"))).unwrap_or_default();
    let mut text = format!(
        "{}\n\n## Your role: {role}\n\n{}\n\n## Your working directory\n\n{}\n",
        seat.prompt.trim(),
        charter.trim(),
        action_dir.display()
    );
    for (path, body) in context {
        text.push_str(&format!("\n## Shared context: {path}\n\n{}\n", body.trim()));
    }
    text.push_str(
        "\n## Working in the hive\n\nYou are one seat of a hive. Use the native hivemind tools to talk to other seats \
         (hivemind_send_agent to message one seat, hivemind_read to read the desk) and hivemind_complete to finish an \
         episode you were assigned. A prose claim of completion is not enough: call the tool.\n\n\
         Turns are asynchronous. After you send a message, END YOUR TURN with a one-line reply and no further \
         tool calls; you are woken automatically when someone replies. Never wait, sleep or poll for replies.\n",
    );
    Ok(text)
}

fn seat_roles(hive_dir: &Path) -> anyhow::Result<BTreeMap<String, String>> {
    let manifest: Value = serde_json::from_str(&std::fs::read_to_string(hive_dir.join("hive.json"))?)?;
    Ok(manifest["seats"]
        .as_array()
        .context("seats")?
        .iter()
        .filter_map(|s| Some((s["id"].as_str()?.to_owned(), s["role"].as_str()?.to_owned())))
        .collect())
}

async fn run(options: Options) -> anyhow::Result<()> {
    let run_dir = std::fs::canonicalize(&options.run_dir).or_else(|_| {
        std::fs::create_dir_all(&options.run_dir)?;
        std::fs::canonicalize(&options.run_dir)
    })?;
    let run_id = match options.run_id.clone() {
        Some(id) => id,
        None => run_dir.file_name().and_then(|n| n.to_str()).context("run id")?.to_owned(),
    };
    let ledger_dir = options.ledger_dir.clone().unwrap_or_else(|| run_dir.clone());
    let swarm_dir = run_dir.join("swarm");
    std::fs::create_dir_all(&swarm_dir)?;
    let trace = Arc::new(Trace::create(&swarm_dir.join("events.jsonl"))?);
    let lowered = package::load(&options.hive)?;
    let roles = seat_roles(&options.hive)?;
    let desk = lowered.desks.first().context("the hive declares no desk")?.clone();
    let tinymemory = std::fs::canonicalize(&options.tinymemory)?;
    let brief = std::fs::read_to_string(&options.brief).context("read brief")?;

    let backend = tinyhivemind_openhuman::offline::backend().await;
    let mut config = tinyhivemind_openhuman::offline::config();
    config.agent.tool_dispatcher = "native".into();
    let gateway = options.gateway.trim_end_matches('/').to_owned();
    let runtime = Runtime::builder()
        .config(config)
        .workspace(Workspace::dir(swarm_dir.join("runtime")))
        .backend_url(backend.uri())
        .provider(Provider::openai_compatible(format!("{gateway}/__tag/swarm/runtime/v1"), "ohrsi-gateway").model("z-ai/glm-5.3-flash"))
        .access(Access::readonly())
        // Seats are coding agents: put the coding pack's schemas on the wire
        // instead of behind `use_skill`.
        .tool_groups(ToolGroups::packed().with("coding", GroupMode::Advertised))
        .build()
        .await?;

    let storage = Arc::new(SqliteStorage::open(swarm_dir.join("hive.db"))?);
    let coordinator = Coordinator::new(runtime.runtime_id().into(), storage, CoordinatorOptions::default()).await?;
    let host = OpenHumanHost::new(runtime.runtime_id().into(), coordinator.clone())?
        .with_turn_timeout(options.turn)?
        .with_hooks(Arc::new(hooks::SwarmHooks { trace: trace.clone() }))?;

    let mut seats = Vec::new();
    let mut candidates = BTreeMap::new();
    for seat in &lowered.seats {
        let role = roles.get(&seat.id).cloned().unwrap_or_default();
        let model = seat.model.clone().unwrap_or_else(|| "z-ai/glm-5.3-flash".into());
        let editing = seat.config["action_dir"] == "{candidate}";
        let action_dir = if editing {
            let wt = worktree::create(&tinymemory, &run_dir, &run_id, &seat.id)?;
            candidates.insert(seat.id.clone(), wt.clone());
            wt
        } else {
            tinymemory.clone()
        };
        let prompt = system_prompt(&options.hive, seat, &role, &lowered.context, &action_dir)?;
        let access = if editing { Access::full() } else { Access::readonly() };
        // The package's `tools.allow` is the seat's whole belt (plus the
        // hivemind tools the adapter attaches); `tools.deny` is removed even
        // if a later belt change would add it.
        let mut definition = AgentDefinitionSpec::new().system_prompt(prompt);
        if !seat.tools.allow.is_empty() {
            let mut allow = seat.tools.allow.clone();
            allow.extend(HIVEMIND_TOOLS.iter().map(|t| (*t).to_owned()));
            definition = definition.tools(ToolScopeSpec::Named(allow));
        }
        if !seat.tools.deny.is_empty() {
            definition = definition.disallow_tools(seat.tools.deny.clone());
        }
        if let Some(n) = seat.config["max_iterations"].as_u64() {
            definition = definition.max_iterations(usize::try_from(n).unwrap_or(usize::MAX));
        }
        let spec = AgentSpec::new(seat.id.clone())
            .definition(definition)
            .provider(Provider::openai_compatible(format!("{gateway}/__tag/swarm/{}/v1", seat.id), "ohrsi-gateway").model(model.clone()))
            .access(access)
            .action_dir(action_dir.clone());
        let agent = runtime.agent(spec)?;
        host.register_agent(agent).await?;
        seats.push(json!({"id": seat.id, "role": role, "model": model, "action_dir": action_dir, "editing": editing}));
    }
    coordinator
        .create_hive(HiveInfo { hive_id: desk.id.clone(), name: desk.name.clone(), description: desk.description.clone(), members: Vec::new() })
        .await?;
    for member in &desk.members {
        coordinator.join_hive(&desk.id, member).await?;
    }
    trace.emit("swarm_started", json!({"hive": lowered.id, "desk": desk.id, "seats": seats, "run": run_id}));

    let lead = desk.members.first().context("empty desk")?.clone();
    let mut kickoff = brief.clone();
    for (seat, path) in &candidates {
        kickoff.push_str(&format!("\n\nCandidate worktree for {seat}: {} (branch rsi/cand-{run_id}-{seat})", path.display()));
    }
    coordinator
        .send_as_host(SendMessage {
            message_id: format!("{run_id}-brief"),
            sender: String::new(),
            destination: Destination::Hive(desk.id.clone()),
            body: kickoff,
            thread: None,
            only_for: vec![lead.clone()],
            starters: Vec::new(),
        })
        .await?;

    let watcher = tokio::spawn(watch(coordinator.clone(), desk.id.clone(), desk.members.clone(), ledger_dir, trace.clone()));
    let outcome = tokio::time::timeout(options.wall, coordinator.run_until_idle()).await;
    tokio::time::sleep(Duration::from_secs(3)).await;
    watcher.abort();
    flush_messages(&coordinator, &desk.id, &desk.members, &mut BTreeSet::new(), &trace);
    let (report, reason) = match outcome {
        Ok(Ok(report)) => (json!({"completed": report.completed, "failed": report.failed, "parked": report.parked}), "idle"),
        Ok(Err(error)) => (json!({"error": error.to_string()}), "coordinator_error"),
        Err(_) => (json!({}), "wall_clock"),
    };
    let mut result = json!({"run": run_id, "reason": reason, "report": report, "candidates": {}});
    for (seat, path) in &candidates {
        result["candidates"][seat] = worktree::summary(path)?;
        result["candidates"][seat]["checkout"] = json!(worktree::candidate_root(&run_dir, seat));
    }
    let mut finished = report.clone();
    finished["reason"] = json!(reason);
    trace.emit("swarm_finished", finished);
    std::fs::write(swarm_dir.join("result.json"), serde_json::to_string_pretty(&result)? + "\n")?;
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}

/// Mirrors new desk and direct messages, and the run's spend, into the trace.
async fn watch(coordinator: Coordinator, desk: String, members: Vec<String>, run_dir: PathBuf, trace: Arc<Trace>) {
    let mut seen = BTreeSet::new();
    let mut meter_offset = 0usize;
    let mut by_seat: BTreeMap<String, f64> = BTreeMap::new();
    loop {
        flush_messages(&coordinator, &desk, &members, &mut seen, &trace);
        if let Ok(text) = std::fs::read_to_string(run_dir.join("meter.jsonl")) {
            for line in text[meter_offset.min(text.len())..].lines() {
                if let Ok(row) = serde_json::from_str::<Value>(line) {
                    let key = if row["phase"] == "swarm" { row["seat"].as_str().unwrap_or("").to_owned() } else { format!("bench:{}", row["phase"].as_str().unwrap_or("")) };
                    *by_seat.entry(key).or_default() += row["cost_usd"].as_f64().unwrap_or(0.0);
                }
            }
            meter_offset = text.len();
        }
        if let (Ok(run), Ok(total)) = (read_json(&run_dir.join("budget.json")), read_json(&run_dir.parent().map(|p| p.join("ledger-total.json")).unwrap_or_default())) {
            let charged = |l: &Value| l["settled_usd"].as_f64().unwrap_or(0.0) + l["uncertain_usd"].as_f64().unwrap_or(0.0) + l["in_flight_usd"].as_f64().unwrap_or(0.0);
            trace.emit("spend", json!({"run_usd": charged(&run), "run_limit_usd": run["limit_usd"], "total_usd": charged(&total),
                "total_limit_usd": total["limit_usd"], "by_seat": by_seat}));
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

fn read_json(path: &Path) -> anyhow::Result<Value> {
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

fn flush_messages(coordinator: &Coordinator, _desk: &str, _members: &[String], seen: &mut BTreeSet<u64>, trace: &Trace) {
    // The host-scope transcript holds desk posts *and* seat-to-seat direct
    // messages, private rows included; agent-scope reads would miss DMs.
    let after = seen.iter().next_back().copied();
    let Ok(messages) = coordinator.read_transcript(after) else {
        return;
    };
    for message in messages {
        if !seen.insert(message.sequence) {
            continue;
        }
        let to = match &message.destination {
            Destination::Hive(id) => format!("hive:{id}"),
            Destination::Agent(id) => id.clone(),
        };
        trace.emit("agent_message", json!({"sequence": message.sequence, "from": message.sender, "to": to,
            "body": clip(&message.body, 4000), "only_for": message.only_for, "episode": message.episode_id}));
    }
}
