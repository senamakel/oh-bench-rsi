//! Unit tests for workspace discovery and inspection.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};

use super::Workspace;
use crate::campaign::Campaign;
use crate::error::Error;

/// A scratch directory in the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("ohrsi-{}-workspace-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("campaigns")).expect("create scratch");
        fs::write(dir.join("Cargo.toml"), "").expect("write manifest");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const CAMPAIGN: &str = r#"
name = "probe"
brief = "probe"
[benchmark]
workdir = "benchmarks"
command = ["true"]
timeout_secs = 1
[metric]
name = "score"
direction = "minimize"
[surface]
editable = ["vendor/openhuman/src/a.rs", "vendor/openhuman/src/b.rs"]
[evaluation]
seeds = [1, 2]
[budget]
usd_cap = 1.0
max_epochs = 1
proposals_per_epoch = 1
"#;

#[test]
fn discovers_the_root_from_a_nested_directory() {
    let scratch = Scratch::new("discover");
    let nested = scratch.0.join("vendor/openhuman/src");
    fs::create_dir_all(&nested).expect("create nested");
    let workspace = Workspace::discover(&nested).expect("found");
    assert_eq!(workspace.root(), scratch.0);
}

#[test]
fn fails_to_discover_outside_a_checkout() {
    let error = Workspace::discover(Path::new("/")).unwrap_err();
    assert!(matches!(error, Error::WorkspaceNotFound { .. }), "{error}");
}

#[test]
fn reports_submodule_population() {
    let scratch = Scratch::new("submodules");
    fs::create_dir_all(scratch.0.join("vendor/openhuman")).expect("empty dir");
    fs::create_dir_all(scratch.0.join("benchmarks")).expect("dir");
    fs::write(scratch.0.join("benchmarks/README.md"), "x").expect("file");
    let status = Workspace::at(&scratch.0).submodules();
    let populated: Vec<_> = status.iter().map(|s| (s.path, s.populated)).collect();
    assert_eq!(
        populated,
        [
            ("vendor/openhuman", false),
            ("vendor/tinyrsi", false),
            ("benchmarks", true)
        ]
    );
}

#[test]
fn lists_only_toml_campaigns_in_order() {
    let scratch = Scratch::new("list");
    for name in ["b.toml", "a.toml", "notes.md"] {
        fs::write(scratch.0.join("campaigns").join(name), "").expect("write");
    }
    let paths = Workspace::at(&scratch.0).campaign_paths().expect("listed");
    let names: Vec<_> = paths
        .iter()
        .map(|p| p.file_name().and_then(|n| n.to_str()).expect("utf-8"))
        .collect();
    assert_eq!(names, ["a.toml", "b.toml"]);
}

#[test]
fn listing_fails_without_a_campaigns_directory() {
    let error = Workspace::at("does/not/exist")
        .campaign_paths()
        .unwrap_err();
    assert!(matches!(error, Error::Io { .. }), "{error}");
}

#[test]
fn finds_missing_surface_paths() {
    let scratch = Scratch::new("missing");
    fs::create_dir_all(scratch.0.join("vendor/openhuman/src")).expect("dir");
    fs::write(scratch.0.join("vendor/openhuman/src/a.rs"), "").expect("file");
    let campaign = Campaign::parse(CAMPAIGN, Path::new("probe.toml")).expect("valid");
    let missing = Workspace::at(&scratch.0).missing_paths(&campaign);
    assert_eq!(missing, ["vendor/openhuman/src/b.rs"]);
}
