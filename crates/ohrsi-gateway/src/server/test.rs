//! Unit tests for option parsing.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::parse;
use crate::error::Error;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn parses_defaults_and_overrides() {
    let options = parse(&args(&["--run-dir", "runs/r1"])).unwrap();
    assert_eq!(options.run_dir, std::path::PathBuf::from("runs/r1"));
    assert!((options.run_limit - 5.0).abs() < 1e-12);
    assert!((options.total_limit - 50.0).abs() < 1e-12);
    assert_eq!(options.listen.port(), 8790);
    let options = parse(&args(&[
        "--run-dir",
        "r",
        "--ledger-total",
        "t.json",
        "--run-limit",
        "2.5",
        "--total-limit",
        "10",
        "--listen",
        "127.0.0.1:0",
        "--upstream",
        "http://127.0.0.1:9/v1",
        "--capture",
    ]))
    .unwrap();
    assert!((options.run_limit - 2.5).abs() < 1e-12);
    assert_eq!(options.ledger_total, std::path::PathBuf::from("t.json"));
    assert_eq!(options.upstream, "http://127.0.0.1:9/v1");
    assert!(options.capture);
}

#[test]
fn rejects_bad_arguments() {
    for bad in [
        &["--run-dir"][..],
        &["--run-dir", "r", "--run-limit", "x"],
        &["--run-dir", "r", "--listen", "nope"],
        &["--run-dir", "r", "--listen", "0.0.0.0:80"],
        &["--bogus"],
        &[],
    ] {
        assert!(
            matches!(parse(&args(bad)).unwrap_err(), Error::Usage(_)),
            "{bad:?}"
        );
    }
}
