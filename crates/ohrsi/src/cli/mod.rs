//! The `ohrsi` command line.
//!
//! [`run`] takes the arguments after the program name and writes to the
//! supplied streams, so the binary is a one-line wrapper and every command is
//! testable without spawning a process.
//!
//! - `ohrsi check [MANIFEST...]` validates the given manifests, or every
//!   manifest in `campaigns/`, and reports uninitialized submodules and
//!   surface paths that do not exist on disk.
//! - `ohrsi list` prints one line per campaign.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::campaign::{Campaign, Direction};
use crate::workspace::Workspace;

/// Exit status for success.
pub const EXIT_OK: u8 = 0;
/// Exit status when a check fails.
pub const EXIT_FAILED: u8 = 1;
/// Exit status for a usage error.
pub const EXIT_USAGE: u8 = 2;

const USAGE: &str = "\
usage: ohrsi <command>

commands:
  check [MANIFEST...]  validate campaigns (default: every campaigns/*.toml)
  list                 list campaigns
  help                 show this message
";

/// Runs one `ohrsi` invocation from `cwd` and returns its exit status.
///
/// Write failures on `out` or `err` are ignored: there is nowhere left to
/// report them.
pub fn run<I, S>(args: I, cwd: &Path, out: &mut dyn Write, err: &mut dyn Write) -> u8
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let args: Vec<String> = args.into_iter().map(Into::into).collect();
    match args.split_first() {
        Some((command, rest)) if command == "check" => check(rest, cwd, out, err),
        Some((command, [])) if command == "list" => list(cwd, out, err),
        Some((command, [])) if command == "help" || command == "--help" || command == "-h" => {
            let _ = out.write_all(USAGE.as_bytes());
            EXIT_OK
        }
        _ => {
            let _ = err.write_all(USAGE.as_bytes());
            EXIT_USAGE
        }
    }
}

fn workspace(cwd: &Path, err: &mut dyn Write) -> Option<Workspace> {
    Workspace::discover(cwd)
        .inspect_err(|error| {
            let _ = writeln!(err, "error: {error}");
        })
        .ok()
}

fn manifests(
    workspace: &Workspace,
    cwd: &Path,
    given: &[String],
    err: &mut dyn Write,
) -> Option<Vec<PathBuf>> {
    if !given.is_empty() {
        return Some(given.iter().map(|path| cwd.join(path)).collect());
    }
    workspace
        .campaign_paths()
        .inspect_err(|error| {
            let _ = writeln!(err, "error: {error}");
        })
        .ok()
}

fn check(given: &[String], cwd: &Path, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let Some(workspace) = workspace(cwd, err) else {
        return EXIT_FAILED;
    };
    let submodules = workspace.submodules();
    for status in submodules.iter().filter(|s| !s.populated) {
        let _ = writeln!(
            err,
            "warning: {} is not checked out; run scripts/bootstrap.sh",
            status.path
        );
    }
    let openhuman_ready = submodules
        .iter()
        .any(|s| s.path == "vendor/openhuman" && s.populated);
    let Some(paths) = manifests(&workspace, cwd, given, err) else {
        return EXIT_FAILED;
    };
    if paths.is_empty() {
        let _ = writeln!(err, "error: no campaign manifests found");
        return EXIT_FAILED;
    }
    let mut failed = false;
    for path in &paths {
        match Campaign::load(path) {
            Ok(campaign) => {
                let missing = workspace.missing_paths(&campaign);
                if openhuman_ready && !missing.is_empty() {
                    failed = true;
                    for path in missing {
                        let _ = writeln!(
                            err,
                            "error: {}: surface path {path} does not exist",
                            campaign.name
                        );
                    }
                } else {
                    let _ = writeln!(out, "ok {}", campaign.name);
                }
            }
            Err(error) => {
                failed = true;
                let _ = writeln!(err, "error: {error}");
            }
        }
    }
    if failed { EXIT_FAILED } else { EXIT_OK }
}

fn list(cwd: &Path, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let Some(workspace) = workspace(cwd, err) else {
        return EXIT_FAILED;
    };
    let Some(paths) = manifests(&workspace, cwd, &[], err) else {
        return EXIT_FAILED;
    };
    let mut status = EXIT_OK;
    for path in paths {
        match Campaign::load(&path) {
            Ok(campaign) => {
                let arrow = match campaign.metric.direction {
                    Direction::Maximize => "max",
                    Direction::Minimize => "min",
                };
                let _ = writeln!(
                    out,
                    "{}\t{} {}\t{}",
                    campaign.name,
                    arrow,
                    campaign.metric.name,
                    campaign.brief.trim().lines().next().unwrap_or_default()
                );
            }
            Err(error) => {
                status = EXIT_FAILED;
                let _ = writeln!(err, "error: {error}");
            }
        }
    }
    status
}

#[cfg(test)]
mod test;
