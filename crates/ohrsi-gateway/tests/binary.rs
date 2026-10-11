//! The `ohrsi-gateway` binary: argument errors, the key requirement, and a
//! clean shutdown on interrupt.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

fn gateway() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ohrsi-gateway"))
}

#[test]
fn rejects_bad_arguments() {
    let out = gateway()
        .arg("--bogus")
        .env("OPENROUTER_API_KEY", "k")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown argument"));
}

#[test]
fn requires_the_provider_key() {
    let out = gateway()
        .args(["--run-dir", "unused"])
        .env_remove("OPENROUTER_API_KEY")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("OPENROUTER_API_KEY"));
}

#[cfg(unix)]
#[test]
fn serves_until_interrupted_and_releases_the_lock() {
    let dir = std::env::temp_dir().join(format!("ohrsi-gw-{}-binary", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut child = gateway()
        .args(["--run-dir"])
        .arg(dir.join("run"))
        .arg("--ledger-total")
        .arg(dir.join("total.json"))
        .args(["--listen", "127.0.0.1:0"])
        .env("OPENROUTER_API_KEY", "k")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert!(line.starts_with("listening on http://127.0.0.1:"), "{line}");
    assert!(dir.join("total.lock").exists());
    let status = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    assert!(child.wait().unwrap().success());
    assert!(
        !dir.join("total.lock").exists(),
        "the lock is released on shutdown"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
