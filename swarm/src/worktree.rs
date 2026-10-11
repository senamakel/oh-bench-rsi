//! Candidate checkouts: one local git worktree per editing seat.
//!
//! Every candidate branches from the incumbent's `rsi/main` in the local
//! TinyMemory repository. Nothing is pushed; branches stay local drafts.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};

/// The candidate checkout root for `seat`: the directory `lme2_eval.py`
/// takes as `--checkout`, holding `vendor/tinymemory`.
pub fn candidate_root(run_dir: &Path, seat: &str) -> PathBuf {
    run_dir.join("candidates").join(seat)
}

/// Creates (or reuses) the seat's TinyMemory worktree on
/// `rsi/cand-<run>-<seat>` from `rsi/main`; returns the worktree path.
pub fn create(tinymemory_repo: &Path, run_dir: &Path, run_id: &str, seat: &str) -> anyhow::Result<PathBuf> {
    let path = candidate_root(run_dir, seat).join("vendor/tinymemory");
    if path.join(".git").exists() {
        return Ok(path);
    }
    let branch = format!("rsi/cand-{run_id}-{seat}");
    std::fs::create_dir_all(path.parent().context("worktree parent")?)?;
    let status = Command::new("git")
        .arg("-C")
        .arg(tinymemory_repo)
        .args(["worktree", "add", "-B", &branch])
        .arg(&path)
        .arg("rsi/main")
        .status()?;
    if !status.success() {
        bail!("git worktree add {branch} failed");
    }
    Ok(path)
}

/// `git diff --stat` and head commit of a candidate against `rsi/main`.
pub fn summary(worktree: &Path) -> anyhow::Result<serde_json::Value> {
    let git = |args: &[&str]| -> anyhow::Result<String> {
        let out = Command::new("git").arg("-C").arg(worktree).args(args).output()?;
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    };
    Ok(serde_json::json!({
        "head": git(&["rev-parse", "HEAD"])?,
        "branch": git(&["branch", "--show-current"])?,
        "commits": git(&["log", "--oneline", "rsi/main..HEAD"])?,
        "diffstat": git(&["diff", "--stat", "rsi/main"])?,
        "uncommitted": git(&["status", "--porcelain"])?,
    }))
}
