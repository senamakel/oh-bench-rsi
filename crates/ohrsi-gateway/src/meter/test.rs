//! Unit tests for the meter.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::{Meter, MeterRow, Tags};
use crate::error::Error;

fn row() -> MeterRow {
    MeterRow {
        at_ms: 1,
        tags: Tags {
            phase: "swarm".into(),
            seat: "lead".into(),
            candidate: String::new(),
        },
        model: "m".into(),
        endpoint: "chat/completions".into(),
        status: 200,
        stream: false,
        reserved_usd: 0.1,
        cost_usd: Some(0.01),
        prompt_tokens: 1,
        completion_tokens: 2,
        wall_ms: 3,
        error: None,
    }
}

#[test]
fn appends_flat_json_lines() {
    let dir = std::env::temp_dir().join(format!("ohrsi-gw-{}-meter", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let meter = Meter::open(&dir.join("nested/meter.jsonl")).unwrap();
    meter.record(&row()).unwrap();
    meter.record(&row()).unwrap();
    let text = std::fs::read_to_string(dir.join("nested/meter.jsonl")).unwrap();
    let lines: Vec<serde_json::Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["seat"], "lead");
    assert_eq!(lines[0]["phase"], "swarm");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reports_unwritable_meters() {
    let dir = std::env::temp_dir().join(format!("ohrsi-gw-{}-meter-bad", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let meter = Meter::open(&dir).unwrap();
    assert!(matches!(
        meter.record(&row()).unwrap_err(),
        Error::Io { .. }
    ));
    let _ = std::fs::remove_dir_all(&dir);
}
