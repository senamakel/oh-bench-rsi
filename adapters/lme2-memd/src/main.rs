//! `lme2-memd`: OpenHuman's memory lifecycle behind loopback HTTP.
//!
//! The LongMemEval-V2 harness runs in a Linux container and calls this
//! daemon through the `oh_tinymemory` `Memory` backend
//! (`adapters/lme2/oh_memory.py`). The daemon owns one TinyMemory engine
//! and one agent memory for one haystack:
//!
//! - `POST /insert`  — one trajectory → one assistant turn per state
//!   (see `convert.rs`), written as accepted bulk stores;
//! - `POST /settle`  — wait until every receipt is readable and, on
//!   CortexDB, until enrichment has been quiet for three polls;
//! - `POST /query`   — `AgentMemory::recall`, the same holistic pack the
//!   product injects before a turn, under OpenHuman's default recall policy;
//! - `GET  /health`.
//!
//! ```text
//! lme2-memd --engine cortex|reference [--listen 127.0.0.1:0] [--namespace lme2-web]
//! ```
//!
//! CortexDB is read from `CORTEX_DB_URL` (loopback) with `CORTEX_API_KEY`.

mod convert;

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};
use tinymemory_api::conformance::ReferenceEngine;
use tinymemory_api::{ItemId, ListRequest, MemoryEngine, WriteOptions};
use tinymemory_integrations::cortex::{CortexCredential, CortexEngine};
use tinymemory_tools::{AgentMemory, MemoryLayout, RecallPolicy};
use tokio::sync::Mutex;

/// The agent id every trajectory is remembered under.
const AGENT: &str = "agent";

/// OpenHuman core's `[memory.recall]` defaults at the pinned revision
/// (`openhuman-core/src/config/schema/memory.rs`), mapped exactly as
/// `memory::lifecycle::policy` maps them. Overridable per field with
/// `MEMD_RECALL_<FIELD>` for ablations.
fn openhuman_policy() -> RecallPolicy {
    let env = |name: &str, default: usize| {
        std::env::var(format!("MEMD_RECALL_{name}"))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    };
    RecallPolicy {
        budget_tokens: env("BUDGET_TOKENS", 1200).max(1),
        learnings_limit: env("LEARNINGS_LIMIT", 8),
        brain_limit: env("BRAIN_LIMIT", 6),
        history_limit: env("HISTORY_LIMIT", 6),
        team_limit: env("TEAM_LIMIT", 3),
        build_beliefs_every: Some(10),
    }
}

struct Daemon {
    engine: Arc<dyn MemoryEngine>,
    layout: MemoryLayout,
    memory: AgentMemory,
    receipts: Mutex<HashSet<ItemId>>,
    cortex: Option<(String, String)>,
}

type Shared = State<Arc<Daemon>>;

fn fail(status: StatusCode, message: impl std::fmt::Display) -> Response {
    (status, Json(json!({ "error": message.to_string() }))).into_response()
}

async fn health(State(d): Shared) -> Response {
    let stored = d.receipts.lock().await.len();
    Json(json!({ "engine": d.engine.descriptor().id, "stored": stored })).into_response()
}

async fn insert(State(d): Shared, Json(trajectory): Json<Value>) -> Response {
    let namespace = match d.layout.conversations(AGENT) {
        Ok(ns) => ns,
        Err(error) => return fail(StatusCode::INTERNAL_SERVER_ERROR, error),
    };
    let items = match convert::trajectory_items(&trajectory, &namespace, AGENT) {
        Ok(items) => items,
        Err(error) => return fail(StatusCode::BAD_REQUEST, error),
    };
    let count = items.len();
    for chunk in items.chunks(tinymemory_api::MAX_STORE_MANY) {
        match d.engine.store_many_with(chunk.to_vec(), WriteOptions::accepted()).await {
            Ok(receipts) => d.receipts.lock().await.extend(receipts.into_iter().map(|r| r.id)),
            Err(error) => return fail(StatusCode::BAD_GATEWAY, error),
        }
    }
    Json(json!({ "items": count })).into_response()
}

async fn readable(d: &Daemon) -> Result<HashSet<ItemId>, String> {
    let mut readable = HashSet::new();
    let mut request = ListRequest::new(d.layout.conversations_filter(Some(AGENT)), 100);
    loop {
        let page = d.engine.list(request.clone()).await.map_err(|e| e.to_string())?;
        readable.extend(page.items.into_iter().map(|hit| hit.id));
        request.cursor = page.next_cursor;
        if request.cursor.is_none() {
            return Ok(readable);
        }
    }
}

async fn settle(State(d): Shared) -> Response {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(1800);
    let expected = d.receipts.lock().await.clone();
    loop {
        match readable(&d).await {
            Ok(found) if expected.is_subset(&found) => break,
            Ok(found) if Instant::now() >= deadline => {
                let n = expected.intersection(&found).count();
                return fail(StatusCode::GATEWAY_TIMEOUT, format!("only {n} of {} items readable", expected.len()));
            }
            Ok(_) => tokio::time::sleep(Duration::from_secs(2)).await,
            Err(error) => return fail(StatusCode::BAD_GATEWAY, error),
        }
    }
    let mut enrichment = Value::Null;
    if let Some((url, key)) = &d.cortex {
        let client = reqwest::Client::new();
        let (mut quiet, mut last) = (0, None);
        while quiet < 3 {
            if Instant::now() >= deadline {
                return fail(StatusCode::GATEWAY_TIMEOUT, format!("enrichment did not drain: {enrichment}"));
            }
            let usage = client.get(format!("{url}/v1/admin/usage")).bearer_auth(key).send().await;
            let usage: Value = match usage {
                Ok(r) => r.json().await.unwrap_or(Value::Null),
                Err(error) => return fail(StatusCode::BAD_GATEWAY, error),
            };
            enrichment = usage["enrichment_backlog"].clone();
            let pending = enrichment["jobs_pending"].as_u64().unwrap_or(u64::MAX);
            let queued = enrichment["jobs_queued_total"].as_u64();
            quiet = if pending == 0 && queued.is_some() && last == queued { quiet + 1 } else { 0 };
            last = queued;
            eprintln!("lme2-memd: enrichment pending={pending} queued={queued:?} quiet={quiet}");
            if quiet < 3 {
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
    Json(json!({ "readable": expected.len(), "enrichment": enrichment, "settle_ms": started.elapsed().as_millis() })).into_response()
}

async fn query(State(d): Shared, Json(body): Json<Value>) -> Response {
    let Some(question) = body["query"].as_str() else {
        return fail(StatusCode::BAD_REQUEST, "missing query");
    };
    let started = Instant::now();
    match d.memory.recall(question).await {
        Ok(pack) => Json(json!({
            "markdown": pack.markdown,
            "tokens": pack.tokens,
            "refs": pack.refs.len(),
            "skipped": pack.skipped,
            "query_ms": started.elapsed().as_millis(),
        }))
        .into_response(),
        Err(error) => fail(StatusCode::BAD_GATEWAY, error),
    }
}

fn router(daemon: Arc<Daemon>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/insert", post(insert))
        .route("/settle", post(settle))
        .route("/query", post(query))
        // Raw trajectories carry every accessibility tree; the largest in
        // the dataset are tens of megabytes.
        .layer(axum::extract::DefaultBodyLimit::max(512 * 1024 * 1024))
        .with_state(daemon)
}

fn build(engine_kind: &str, namespace: &str) -> Result<Daemon, String> {
    let (engine, cortex): (Arc<dyn MemoryEngine>, _) = match engine_kind {
        "reference" => (Arc::new(ReferenceEngine::new()), None),
        "cortex" => {
            let url = std::env::var("CORTEX_DB_URL").map_err(|_| "CORTEX_DB_URL is not set".to_string())?;
            let key = std::env::var("CORTEX_API_KEY").unwrap_or_else(|_| "lme2-local".into());
            let engine = CortexEngine::direct(&url, CortexCredential::api_key(key.clone())).map_err(|e| e.to_string())?;
            (Arc::new(engine), Some((url, key)))
        }
        other => return Err(format!("unknown engine {other:?}")),
    };
    let layout = MemoryLayout::new(format!("project:{namespace}").parse().map_err(|e| format!("{e}"))?)
        .map_err(|e| e.to_string())?;
    let memory = AgentMemory::new(engine.clone(), layout.clone(), AGENT)
        .map_err(|e| e.to_string())?
        .with_policy(openhuman_policy());
    Ok(Daemon { engine, layout, memory, receipts: Mutex::default(), cortex })
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let engine = flag("--engine").unwrap_or_else(|| "cortex".into());
    let namespace = flag("--namespace").unwrap_or_else(|| "lme2".into());
    let listen: SocketAddr = match flag("--listen").unwrap_or_else(|| "127.0.0.1:0".into()).parse() {
        Ok(addr) => addr,
        Err(error) => {
            eprintln!("lme2-memd: --listen: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let daemon = match build(&engine, &namespace) {
        Ok(daemon) => Arc::new(daemon),
        Err(error) => {
            eprintln!("lme2-memd: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let listener = match tokio::net::TcpListener::bind(listen).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("lme2-memd: bind {listen}: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };
    match listener.local_addr() {
        Ok(addr) => println!("listening on http://{addr}"),
        Err(error) => eprintln!("lme2-memd: {error}"),
    }
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    if let Err(error) = axum::serve(listener, router(daemon)).with_graceful_shutdown(shutdown).await {
        eprintln!("lme2-memd: {error}");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}

#[cfg(test)]
mod daemon_test {
    use super::*;

    async fn call(app: &Router, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
        use tower_like::oneshot;
        oneshot(app.clone(), method, path, body).await
    }

    /// Minimal request driver so the test needs no extra dependency.
    mod tower_like {
        use super::*;
        pub async fn oneshot(app: Router, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
            let addr = listener.local_addr().expect("addr");
            let server = tokio::spawn(async move { axum::serve(listener, app).await.expect("serve") });
            let client = reqwest::Client::new();
            let url = format!("http://{addr}{path}");
            let response = if method == "GET" { client.get(url).send().await } else { client.post(url).json(&body).send().await }
                .expect("request");
            let status = StatusCode::from_u16(response.status().as_u16()).expect("status");
            let value = response.json().await.unwrap_or(Value::Null);
            server.abort();
            (status, value)
        }
    }

    #[tokio::test]
    async fn round_trips_a_trajectory_through_the_reference_engine() {
        let app = router(Arc::new(build("reference", "lme2-test").expect("daemon")));
        let trajectory = json!({"id": "t1", "goal": "open the incident filters", "outcome": "success", "states": [
            {"url": "http://x/incidents", "thought": "open filters", "action": "click('5')",
             "accessibility_tree": "[5] button 'Filters'\n[6] option 'Incident Portal'\n[7] option 'My Open Incidents'"}
        ]});
        let (status, body) = call(&app, "POST", "/insert", trajectory).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["items"], 1);
        let (status, body) = call(&app, "POST", "/settle", json!({})).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["readable"], 1);
        let (status, body) = call(&app, "POST", "/query", json!({"query": "Which filter options mention Incident?"})).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(body["markdown"].as_str().expect("markdown").contains("Incident Portal"), "{body}");
        let (status, body) = call(&app, "GET", "/health", json!({})).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["stored"], 1);
    }

    #[tokio::test]
    async fn rejects_bad_requests() {
        let app = router(Arc::new(build("reference", "lme2-test").expect("daemon")));
        assert_eq!(call(&app, "POST", "/insert", json!({"states": []})).await.0, StatusCode::BAD_REQUEST);
        assert_eq!(call(&app, "POST", "/query", json!({})).await.0, StatusCode::BAD_REQUEST);
        assert!(build("nope", "x").is_err());
    }
}
