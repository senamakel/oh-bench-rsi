//! Turn hooks that stream each seat's tool calls and usage into the trace.

use std::sync::Arc;

use openhuman_embed::agent_progress::AgentProgress;
use serde_json::json;
use tinyhivemind_hives::TurnDisposition;
use tinyhivemind_openhuman::{TurnHooks, TurnProgressSink, TurnScope};

use crate::trace::{Trace, clip};

pub struct SwarmHooks {
    pub trace: Arc<Trace>,
}

impl TurnHooks for SwarmHooks {
    fn progress(&self, scope: &TurnScope) -> Option<TurnProgressSink> {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<AgentProgress>(256);
        let trace = self.trace.clone();
        let seat = scope.agent_id.clone();
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    AgentProgress::ToolCallStarted { tool_name, arguments, display_detail, .. } => {
                        let detail = display_detail.unwrap_or_else(|| clip(&arguments.to_string(), 240));
                        trace.emit("tool_call", json!({"seat": seat, "tool": tool_name, "detail": detail, "phase": "started"}));
                    }
                    AgentProgress::ToolCallCompleted { tool_name, success, output_chars, .. } => {
                        trace.emit("tool_call", json!({"seat": seat, "tool": tool_name, "phase": "completed",
                            "success": success, "output_chars": output_chars}));
                    }
                    _ => {}
                }
            }
        });
        Some(tx)
    }

    fn after_turn(
        &self,
        scope: &TurnScope,
        usage: Option<&openhuman_core::agent::tinyagents::host::LastTurnUsage>,
    ) -> tinyhivemind_openhuman::Result<TurnDisposition> {
        let (input, output, cost) = usage.map_or((0, 0, 0.0), |u| (u.input_tokens, u.output_tokens, u.cost_usd));
        self.trace.emit("agent_turn", json!({"seat": scope.agent_id, "input_tokens": input,
            "output_tokens": output, "cost_usd": cost}));
        Ok(TurnDisposition::Completed)
    }
}
