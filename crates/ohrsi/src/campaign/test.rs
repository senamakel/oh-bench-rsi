//! Unit tests for campaign parsing and validation.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use super::{Campaign, Direction};
use crate::error::Error;

const VALID: &str = r#"
name = "memory-recall"
brief = "Raise LongMemEval accuracy without regressing LoCoMo."

[benchmark]
workdir = "benchmarks"
command = ["node", "memory/run.mjs"]
env = ["OPENROUTER_API_KEY"]
timeout_secs = 3600

[metric]
name = "lme_accuracy"
direction = "maximize"
target = 0.7

[surface]
editable = ["vendor/openhuman/vendor/tinymemory/crates/tinymemory-tools/src/recall"]
frozen = ["vendor/openhuman/vendor/tinymemory/crates/tinymemory-api"]

[evaluation]
seeds = [27, 28]

[budget]
usd_cap = 10.0
max_epochs = 5
proposals_per_epoch = 2
"#;

fn parse(text: &str) -> Result<Campaign, Error> {
    Campaign::parse(text, Path::new("campaigns/test.toml"))
}

fn with(from: &str, to: &str) -> String {
    assert!(VALID.contains(from), "fixture does not contain {from:?}");
    VALID.replacen(from, to, 1)
}

#[test]
fn parses_a_valid_manifest() {
    let campaign = parse(VALID).expect("fixture is valid");
    assert_eq!(campaign.name, "memory-recall");
    assert_eq!(campaign.metric.direction, Direction::Maximize);
    assert_eq!(campaign.metric.target, Some(0.7));
    assert_eq!(campaign.benchmark.env, ["OPENROUTER_API_KEY"]);
    assert_eq!(campaign.evaluation.seeds, [27, 28]);
}

#[test]
fn always_freezes_the_benchmarks() {
    let campaign = parse(VALID).expect("fixture is valid");
    assert_eq!(
        campaign.frozen_paths(),
        [
            "benchmarks",
            "vendor/openhuman/vendor/tinymemory/crates/tinymemory-api"
        ]
    );
}

#[test]
fn does_not_list_benchmarks_twice_when_declared() {
    let campaign = parse(&with(
        "frozen = [\"vendor/openhuman/vendor/tinymemory/crates/tinymemory-api\"]",
        "frozen = [\"benchmarks\"]",
    ))
    .expect("valid");
    assert_eq!(campaign.frozen_paths(), ["benchmarks"]);
}

#[test]
fn rejects_unknown_fields() {
    let error = parse(&with("[metric]", "[metric]\nweight = 2")).unwrap_err();
    assert!(matches!(error, Error::Parse { .. }), "{error}");
}

#[test]
fn rejects_a_name_that_is_not_kebab_case() {
    for name in [
        "Memory",
        "",
        "-lead",
        "trail-",
        "two--dashes",
        "under_score",
    ] {
        let error = parse(&with("\"memory-recall\"", &format!("{name:?}"))).unwrap_err();
        assert!(matches!(error, Error::InvalidName(_)), "{name}: {error}");
    }
}

#[test]
fn rejects_an_empty_command() {
    let error = parse(&with("[\"node\", \"memory/run.mjs\"]", "[]")).unwrap_err();
    assert!(matches!(error, Error::EmptyCommand));
    let error = parse(&with("[\"node\", \"memory/run.mjs\"]", "[\"\"]")).unwrap_err();
    assert!(matches!(error, Error::EmptyCommand));
}

#[test]
fn rejects_an_unsafe_workdir() {
    for workdir in ["/abs", "../up", "a/./b", "", "a\\\\b"] {
        let error = parse(&with(
            "\"benchmarks\"\ncommand",
            &format!("{workdir:?}\ncommand"),
        ))
        .unwrap_err();
        assert!(matches!(error, Error::UnsafePath(_)), "{workdir}: {error}");
    }
}

#[test]
fn accepts_the_repository_root_as_workdir() {
    let campaign = parse(&with("\"benchmarks\"\ncommand", "\".\"\ncommand")).expect("valid");
    assert_eq!(campaign.benchmark.workdir, ".");
}

#[test]
fn rejects_an_empty_metric_name() {
    let error = parse(&with("\"lme_accuracy\"", "\" \"")).unwrap_err();
    assert!(matches!(error, Error::EmptyMetricName));
}

#[test]
fn rejects_a_non_finite_target() {
    let error = parse(&with("target = 0.7", "target = nan")).unwrap_err();
    assert!(matches!(error, Error::NonFiniteTarget));
}

#[test]
fn accepts_a_missing_target() {
    let campaign = parse(&with("target = 0.7\n", "")).expect("target is optional");
    assert_eq!(campaign.metric.target, None);
}

#[test]
fn rejects_an_empty_editable_surface() {
    let error = parse(&with(
        "editable = [\"vendor/openhuman/vendor/tinymemory/crates/tinymemory-tools/src/recall\"]",
        "editable = []",
    ))
    .unwrap_err();
    assert!(matches!(error, Error::NoEditable));
}

#[test]
fn rejects_editable_paths_outside_openhuman() {
    for path in [
        "benchmarks/memory",
        "vendor/tinyrsi/crates",
        "vendor/openhumanx/src",
        "vendor/openhuman/",
    ] {
        let error = parse(&with(
            "\"vendor/openhuman/vendor/tinymemory/crates/tinymemory-tools/src/recall\"",
            &format!("{path:?}"),
        ))
        .unwrap_err();
        assert!(
            matches!(error, Error::OutsideOpenHuman(_)),
            "{path}: {error}"
        );
    }
}

#[test]
fn rejects_editable_paths_that_escape() {
    let error = parse(&with(
        "\"vendor/openhuman/vendor/tinymemory/crates/tinymemory-tools/src/recall\"",
        "\"vendor/openhuman/../../benchmarks\"",
    ))
    .unwrap_err();
    assert!(matches!(error, Error::UnsafePath(_)), "{error}");
}

#[test]
fn rejects_unsafe_frozen_paths() {
    let error = parse(&with(
        "frozen = [\"vendor/openhuman/vendor/tinymemory/crates/tinymemory-api\"]",
        "frozen = [\"/etc\"]",
    ))
    .unwrap_err();
    assert!(matches!(error, Error::UnsafePath(_)), "{error}");
}

#[test]
fn rejects_editable_paths_overlapping_frozen_ones() {
    let inside = with(
        "\"vendor/openhuman/vendor/tinymemory/crates/tinymemory-tools/src/recall\"",
        "\"vendor/openhuman/vendor/tinymemory/crates/tinymemory-api/src/lib.rs\"",
    );
    let error = parse(&inside).unwrap_err();
    assert!(matches!(error, Error::FrozenEditable { .. }), "{error}");

    let enclosing = with(
        "\"vendor/openhuman/vendor/tinymemory/crates/tinymemory-tools/src/recall\"",
        "\"vendor/openhuman/vendor/tinymemory\"",
    );
    let error = parse(&enclosing).unwrap_err();
    assert!(matches!(error, Error::FrozenEditable { .. }), "{error}");
}

#[test]
fn rejects_too_few_seeds() {
    let error = parse(&with("seeds = [27, 28]", "seeds = [27]")).unwrap_err();
    assert!(matches!(error, Error::TooFewSeeds(1)));
}

#[test]
fn rejects_duplicate_seeds() {
    let error = parse(&with("seeds = [27, 28]", "seeds = [27, 28, 27]")).unwrap_err();
    assert!(matches!(error, Error::DuplicateSeed(27)));
}

#[test]
fn rejects_a_bad_spend_cap() {
    for cap in ["0.0", "-1.0", "inf"] {
        let error = parse(&with("usd_cap = 10.0", &format!("usd_cap = {cap}"))).unwrap_err();
        assert!(matches!(error, Error::InvalidSpendCap), "{cap}: {error}");
    }
}

#[test]
fn rejects_zero_limits() {
    let error = parse(&with("max_epochs = 5", "max_epochs = 0")).unwrap_err();
    assert!(matches!(error, Error::ZeroLimit("max_epochs")));
    let error = parse(&with("proposals_per_epoch = 2", "proposals_per_epoch = 0")).unwrap_err();
    assert!(matches!(error, Error::ZeroLimit("proposals_per_epoch")));
}

#[test]
fn load_reports_a_missing_file() {
    let error = Campaign::load(Path::new("does/not/exist.toml")).unwrap_err();
    assert!(matches!(error, Error::Io { .. }), "{error}");
}
