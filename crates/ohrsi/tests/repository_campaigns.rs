//! The campaigns committed to this repository must validate, and the `ohrsi`
//! binary must report them.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::process::Command;

use ohrsi::{Campaign, Workspace};

fn workspace() -> Workspace {
    Workspace::discover(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("crate lives in the checkout")
}

#[test]
fn every_committed_campaign_validates() {
    let paths = workspace()
        .campaign_paths()
        .expect("campaigns/ is readable");
    assert!(
        !paths.is_empty(),
        "the repository ships at least one campaign"
    );
    for path in paths {
        Campaign::load(&path).unwrap_or_else(|error| panic!("{error}"));
    }
}

#[test]
fn committed_campaigns_never_edit_the_benchmarks() {
    for path in workspace().campaign_paths().expect("readable") {
        let campaign = Campaign::load(&path).expect("valid");
        assert!(campaign.frozen_paths().contains(&"benchmarks"));
        assert!(
            campaign
                .surface
                .editable
                .iter()
                .all(|p| !p.starts_with("benchmarks"))
        );
    }
}

#[test]
fn the_binary_lists_the_committed_campaigns() {
    let output = Command::new(env!("CARGO_BIN_EXE_ohrsi"))
        .arg("list")
        .current_dir(workspace().root())
        .output()
        .expect("binary runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf-8");
    assert!(
        stdout.lines().any(|line| line.starts_with("lme2-recall\t")),
        "{stdout}"
    );
}

#[test]
fn the_binary_rejects_an_unknown_command() {
    let status = Command::new(env!("CARGO_BIN_EXE_ohrsi"))
        .arg("frobnicate")
        .status()
        .expect("binary runs");
    assert_eq!(status.code(), Some(2));
}
