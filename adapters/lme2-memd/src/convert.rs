//! Turning one LongMemEval-V2 trajectory into OpenHuman memory writes.
//!
//! A trajectory is a browsing agent's episode: a goal, an outcome, and a
//! list of states, each with the URL, the agent's thought, the action it
//! took, and the page's accessibility tree. OpenHuman remembers an agent's
//! work as conversation turns, so each state becomes one assistant turn in
//! a thread named after the trajectory. The turn carries:
//!
//! - the task (abbreviated after the first state) and the outcome;
//! - the URL, thought and action;
//! - the page content **new to this trajectory** — accessibility-tree lines
//!   not already stored for an earlier state, with element ids stripped and
//!   the total capped. Most of a page repeats from state to state; storing
//!   only what changed keeps the haystack to about a sixth of its raw size
//!   without dropping any line the agent saw.

use std::collections::HashSet;

use serde_json::Value;
use tinymemory_api::{MemoryMeta, Namespace, Role, StoreItem, Turn, TurnRange};

/// Most characters of new page content stored per state.
pub const PAGE_CAP_CHARS: usize = 6_000;
/// Most characters of the goal repeated after the first state.
pub const GOAL_ECHO_CHARS: usize = 160;

/// Why a trajectory could not be converted.
#[derive(Debug, PartialEq, Eq)]
pub enum ConvertError {
    /// A required field is missing or has the wrong type.
    Field(&'static str),
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Field(name) => write!(f, "trajectory field {name} is missing or invalid"),
        }
    }
}

/// The memory writes for one trajectory, one per state, in order.
///
/// # Errors
///
/// Returns [`ConvertError::Field`] if `id` or `states` is missing.
pub fn trajectory_items(trajectory: &Value, namespace: &Namespace, agent_id: &str) -> Result<Vec<StoreItem>, ConvertError> {
    let id = trajectory["id"].as_str().ok_or(ConvertError::Field("id"))?;
    let states = trajectory["states"].as_array().ok_or(ConvertError::Field("states"))?;
    let goal = goal_text(&trajectory["goal"]);
    let outcome = trajectory["outcome"].as_str().unwrap_or("unknown");
    let mut seen = HashSet::new();
    let mut items = Vec::with_capacity(states.len());
    for (index, state) in states.iter().enumerate() {
        let task = if index == 0 { goal.clone() } else { abbreviate(&goal, GOAL_ECHO_CHARS) };
        let mut text = format!("Task: {task}\nOutcome of this task: {outcome}\nStep {index}");
        if let Some(url) = state["url"].as_str() {
            text.push_str(&format!("\nURL: {url}"));
        }
        if let Some(thought) = state["thought"].as_str().filter(|t| !t.trim().is_empty()) {
            text.push_str(&format!("\nThought: {}", thought.trim()));
        }
        if let Some(action) = state["action"].as_str().filter(|a| !a.trim().is_empty()) {
            text.push_str(&format!("\nAction: {}", action.trim()));
        }
        let page = novel_page_lines(state["accessibility_tree"].as_str().unwrap_or(""), &mut seen);
        if !page.is_empty() {
            text.push_str("\nPage content (new):\n");
            text.push_str(&page);
        }
        let index32 = u32::try_from(index).unwrap_or(u32::MAX);
        let meta = MemoryMeta {
            namespace: namespace.clone(),
            agent_id: Some(agent_id.to_owned()),
            thread_id: Some(format!("trajectory-{id}")),
            turns: Some(TurnRange { first: index32, last: index32 }),
            ..Default::default()
        };
        items.push(StoreItem::Conversation {
            turns: vec![Turn::new(Role::Assistant, text)],
            meta,
        });
    }
    Ok(items)
}

fn goal_text(goal: &Value) -> String {
    match goal {
        Value::String(text) => text.trim().to_owned(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

fn abbreviate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut short: String = text.chars().take(max).collect();
    short.push('…');
    short
}

/// Lines of `tree` not in `seen`, element ids stripped, capped in total.
fn novel_page_lines(tree: &str, seen: &mut HashSet<String>) -> String {
    let mut out = String::new();
    for raw in tree.lines() {
        let line = strip_element_id(raw.trim());
        if line.is_empty() || !seen.insert(line.clone()) {
            continue;
        }
        if out.len() + line.len() + 1 > PAGE_CAP_CHARS {
            break;
        }
        out.push_str(&line);
        out.push('\n');
    }
    out.truncate(out.trim_end().len());
    out
}

/// `[123] link 'Home'` → `link 'Home'`.
fn strip_element_id(line: &str) -> String {
    if let Some(rest) = line.strip_prefix('[') {
        if let Some((digits, tail)) = rest.split_once(']') {
            if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
                return tail.trim_start().to_owned();
            }
        }
    }
    line.to_owned()
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;

    fn namespace() -> Namespace {
        "project:lme2-test".parse().expect("valid namespace")
    }

    fn text(item: &StoreItem) -> &str {
        match item {
            StoreItem::Conversation { turns, .. } => &turns[0].text,
            _ => panic!("expected a conversation"),
        }
    }

    #[test]
    fn stores_one_turn_per_state_with_only_new_page_lines() {
        let trajectory = json!({
            "id": "t1", "goal": ["Find", "the filter"], "outcome": "success",
            "states": [
                {"url": "http://x/a", "thought": "look", "action": "click('1')",
                 "accessibility_tree": "RootWebArea 'A'\n\t[12] link 'Home'\n\t[13] button 'Filters'"},
                {"url": "http://x/b", "thought": "", "action": null,
                 "accessibility_tree": "RootWebArea 'A'\n\t[12] link 'Home'\n\t[40] option 'Incident Portal'"}
            ]
        });
        let items = trajectory_items(&trajectory, &namespace(), "agent").expect("converts");
        assert_eq!(items.len(), 2);
        let first = text(&items[0]);
        assert!(first.contains("Task: Find the filter"));
        assert!(first.contains("Action: click('1')"));
        assert!(first.contains("link 'Home'"));
        assert!(!first.contains("[12]"));
        let second = text(&items[1]);
        assert!(second.contains("option 'Incident Portal'"));
        assert!(!second.contains("link 'Home'"), "repeated lines are not stored twice");
        assert!(!second.contains("Thought:"));
        match &items[1] {
            StoreItem::Conversation { meta, .. } => {
                assert_eq!(meta.thread_id.as_deref(), Some("trajectory-t1"));
                assert_eq!(meta.turns, Some(TurnRange { first: 1, last: 1 }));
            }
            _ => panic!("expected a conversation"),
        }
    }

    #[test]
    fn caps_page_content_and_abbreviates_the_repeated_goal() {
        let tree: String = (0..5000).map(|i| format!("StaticText 'row {i}'\n")).collect();
        let goal = "g".repeat(400);
        let trajectory = json!({"id": "t", "goal": goal, "states": [{"accessibility_tree": tree}, {}]});
        let items = trajectory_items(&trajectory, &namespace(), "agent").expect("converts");
        assert!(text(&items[0]).len() < PAGE_CAP_CHARS + 600);
        assert!(text(&items[1]).contains('…'));
        assert!(text(&items[1]).contains("Outcome of this task: unknown"));
    }

    #[test]
    fn rejects_trajectories_without_id_or_states() {
        assert_eq!(trajectory_items(&json!({"states": []}), &namespace(), "a").unwrap_err(), ConvertError::Field("id"));
        assert_eq!(trajectory_items(&json!({"id": "x"}), &namespace(), "a").unwrap_err(), ConvertError::Field("states"));
        assert_eq!(ConvertError::Field("id").to_string(), "trajectory field id is missing or invalid");
    }

    #[test]
    fn keeps_bracketed_text_that_is_not_an_element_id() {
        assert_eq!(strip_element_id("[a] x"), "[a] x");
        assert_eq!(strip_element_id("[] x"), "[] x");
        assert_eq!(strip_element_id("[7] x"), "x");
        assert_eq!(goal_text(&json!(5)), "");
    }
}
