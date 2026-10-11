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
        &["--run-dir", "r", "--ledger-total"],
        &["--run-dir", "r", "--run-limit"],
        &["--run-dir", "r", "--total-limit"],
        &["--run-dir", "r", "--listen"],
        &["--run-dir", "r", "--upstream"],
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

fn options(dir: &std::path::Path, run_limit: f64) -> super::Options {
    super::Options {
        run_dir: dir.join("run"),
        ledger_total: dir.join("total.json"),
        run_limit,
        total_limit: 50.0,
        listen: "127.0.0.1:0".parse().unwrap(),
        upstream: "http://127.0.0.1:9/v1".into(),
        capture: false,
    }
}

#[tokio::test]
async fn opening_fails_on_a_bad_ledger_meter_or_address() {
    let dir = std::env::temp_dir().join(format!("ohrsi-gw-{}-server-open", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let error = super::Bound::open(&options(&dir, 0.0), "k".into())
        .await
        .unwrap_err();
    assert!(matches!(error, Error::InvalidLimit(_)), "{error}");

    let mut bad_total = options(&dir, 5.0);
    bad_total.total_limit = -1.0;
    bad_total.ledger_total = dir.join("total2.json");
    let error = super::Bound::open(&bad_total, "k".into())
        .await
        .unwrap_err();
    assert!(matches!(error, Error::InvalidLimit(_)), "{error}");

    let mut bad_meter = options(&dir, 5.0);
    bad_meter.ledger_total = dir.join("total3.json");
    bad_meter.run_dir = dir.join("run3");
    std::fs::create_dir_all(bad_meter.run_dir.join("meter.jsonl")).unwrap();
    std::fs::write(dir.join("blocker"), "").unwrap();
    bad_meter.run_dir = dir.join("blocker").join("run");
    let error = super::Bound::open(&bad_meter, "k".into())
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Io { .. }), "{error}");

    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut busy = options(&dir, 5.0);
    busy.ledger_total = dir.join("total4.json");
    busy.run_dir = dir.join("run4");
    busy.listen = taken.local_addr().unwrap();
    let error = super::Bound::open(&busy, "k".into()).await.unwrap_err();
    assert!(matches!(error, Error::Io { .. }), "{error}");
    let _ = std::fs::remove_dir_all(&dir);
}
