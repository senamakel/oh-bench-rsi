//! Unit tests for the command line.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};

use super::{EXIT_FAILED, EXIT_OK, EXIT_USAGE, run};

/// A scratch checkout in the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("ohrsi-{}-cli-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("campaigns")).expect("create scratch");
        fs::write(dir.join("Cargo.toml"), "").expect("write manifest");
        Self(dir)
    }

    fn campaign(&self, file: &str, name: &str, editable: &str) -> &Self {
        let text = format!(
            r#"
name = "{name}"
brief = "Make {name} better."
[benchmark]
workdir = "benchmarks"
command = ["true"]
timeout_secs = 1
[metric]
name = "score"
direction = "maximize"
[surface]
editable = ["{editable}"]
[evaluation]
seeds = [1, 2]
[budget]
usd_cap = 1.0
max_epochs = 1
proposals_per_epoch = 1
"#
        );
        fs::write(self.0.join("campaigns").join(file), text).expect("write campaign");
        self
    }

    fn populate_openhuman(&self) -> &Self {
        let dir = self.0.join("vendor/openhuman/src");
        fs::create_dir_all(&dir).expect("create");
        fs::write(dir.join("present.rs"), "").expect("write");
        self
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn invoke(args: &[&str], cwd: &Path) -> (u8, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(args.iter().copied(), cwd, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).expect("utf-8"),
        String::from_utf8(err).expect("utf-8"),
    )
}

#[test]
fn prints_usage_on_help() {
    let (code, out, _) = invoke(&["help"], Path::new("."));
    assert_eq!(code, EXIT_OK);
    assert!(out.starts_with("usage: ohrsi"));
}

#[test]
fn rejects_unknown_and_missing_commands() {
    for args in [&[][..], &["frobnicate"][..], &["list", "extra"][..]] {
        let (code, _, err) = invoke(args, Path::new("."));
        assert_eq!(code, EXIT_USAGE, "{args:?}");
        assert!(err.starts_with("usage: ohrsi"));
    }
}

#[test]
fn check_passes_valid_campaigns_and_warns_about_submodules() {
    let scratch = Scratch::new("check-ok");
    scratch.campaign("a.toml", "alpha", "vendor/openhuman/src/absent.rs");
    let (code, out, err) = invoke(&["check"], &scratch.0);
    assert_eq!(code, EXIT_OK, "{err}");
    assert_eq!(out, "ok alpha\n");
    assert!(err.contains("vendor/openhuman is not checked out"), "{err}");
}

#[test]
fn check_fails_on_missing_surface_paths_once_openhuman_is_present() {
    let scratch = Scratch::new("check-missing");
    scratch
        .populate_openhuman()
        .campaign("a.toml", "alpha", "vendor/openhuman/src/absent.rs")
        .campaign("b.toml", "beta", "vendor/openhuman/src/present.rs");
    let (code, out, err) = invoke(&["check"], &scratch.0);
    assert_eq!(code, EXIT_FAILED);
    assert_eq!(out, "ok beta\n");
    assert!(
        err.contains("alpha: surface path vendor/openhuman/src/absent.rs does not exist"),
        "{err}"
    );
}

#[test]
fn check_accepts_explicit_manifests_relative_to_cwd() {
    let scratch = Scratch::new("check-explicit");
    scratch
        .campaign("a.toml", "alpha", "vendor/openhuman/src/a.rs")
        .campaign("b.toml", "beta", "benchmarks/memory");
    let (code, out, err) = invoke(&["check", "a.toml"], &scratch.0.join("campaigns"));
    assert_eq!(code, EXIT_OK, "{err}");
    assert_eq!(out, "ok alpha\n");
}

#[test]
fn check_reports_invalid_manifests() {
    let scratch = Scratch::new("check-invalid");
    scratch.campaign("a.toml", "alpha", "benchmarks/memory");
    let (code, _, err) = invoke(&["check"], &scratch.0);
    assert_eq!(code, EXIT_FAILED);
    assert!(err.contains("must lie under vendor/openhuman/"), "{err}");
}

#[test]
fn check_fails_with_no_manifests() {
    let scratch = Scratch::new("check-empty");
    let (code, _, err) = invoke(&["check"], &scratch.0);
    assert_eq!(code, EXIT_FAILED);
    assert!(err.contains("no campaign manifests found"), "{err}");
}

#[test]
fn commands_fail_outside_a_checkout() {
    for command in ["check", "list"] {
        let (code, _, err) = invoke(&[command], Path::new("/"));
        assert_eq!(code, EXIT_FAILED, "{command}");
        assert!(err.contains("no oh-bench-rsi workspace"), "{err}");
    }
}

#[test]
fn list_prints_one_line_per_campaign_and_flags_bad_ones() {
    let scratch = Scratch::new("list");
    scratch
        .campaign("a.toml", "alpha", "vendor/openhuman/src/a.rs")
        .campaign("b.toml", "beta", "benchmarks/memory");
    let (code, out, err) = invoke(&["list"], &scratch.0);
    assert_eq!(code, EXIT_FAILED);
    assert_eq!(out, "alpha\tmax score\tMake alpha better.\n");
    assert!(err.contains("must lie under vendor/openhuman/"), "{err}");
}
