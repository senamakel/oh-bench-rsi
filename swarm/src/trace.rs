//! The swarm's live trace: `events.jsonl` in tinyrsi's loop-trace envelope.
//!
//! Each line is `{"seq": n, "elapsed_ms": t, "event": {"kind": ..}}`, the
//! envelope `apps/rsi-viz` already follows. The swarm adds these kinds:
//!
//! | kind | fields |
//! | --- | --- |
//! | `swarm_started` | `hive`, `seats` (id, role, model, action_dir), `desk` |
//! | `agent_message` | `sequence`, `from`, `to` (`hive:<id>` or agent), `body`, `only_for` |
//! | `tool_call` | `seat`, `tool`, `detail`, `phase` (`started`/`completed`), `success` |
//! | `agent_turn` | `seat`, `input_tokens`, `output_tokens`, `cost_usd` |
//! | `spend` | `run_usd`, `run_limit_usd`, `total_usd`, `total_limit_usd`, `by_seat` |
//! | `swarm_finished` | `completed`, `failed`, `parked`, `reason` |

use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use serde_json::{Value, json};

/// An append-only, thread-safe JSONL trace.
pub struct Trace {
    file: Mutex<(std::fs::File, u64)>,
    started: Instant,
}

impl Trace {
    pub fn create(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self { file: Mutex::new((file, 0)), started: Instant::now() })
    }

    pub fn emit(&self, kind: &str, mut fields: Value) {
        if let Value::Object(map) = &mut fields {
            map.insert("kind".into(), Value::String(kind.into()));
        }
        let mut guard = self.file.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let seq = guard.1;
        guard.1 += 1;
        let line = json!({ "seq": seq, "elapsed_ms": self.started.elapsed().as_millis(), "event": fields });
        let _ = writeln!(guard.0, "{line}");
    }
}

/// Shortens `text` to `max` characters for the trace.
pub fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}
