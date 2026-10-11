//! # ohrsi
//!
//! The driver for recursive self-improvement (RSI) of OpenHuman's memory, as
//! measured by the pinned memory benchmarks in `benchmarks/`.
//!
//! This repository vendors three checkouts: OpenHuman (`vendor/openhuman`, the
//! code being improved), tinyrsi (`vendor/tinyrsi`, the improvement loop), and
//! openhuman-benchmarks (`benchmarks/`, the frozen measurement). A *campaign*
//! ties them together: which benchmark to run, which metric to move, which
//! OpenHuman paths a proposal may edit, and the spend and epoch limits.
//!
//! The entry points today are:
//!
//! - [`Campaign`] — load and validate a `campaigns/*.toml` manifest;
//! - [`Workspace`] — locate the checkout and inspect its submodules;
//! - [`cli::run`] — the `ohrsi` command line.
//!
//! ```
//! use std::path::Path;
//! use ohrsi::{Campaign, Direction};
//!
//! let manifest = r#"
//! name = "recall-pack"
//! brief = "Keep retrieved decisions in the rendered memory pack."
//! [benchmark]
//! workdir = "benchmarks"
//! command = ["node", "memory/run.mjs"]
//! timeout_secs = 3600
//! [metric]
//! name = "lme_accuracy"
//! direction = "maximize"
//! [surface]
//! editable = ["vendor/openhuman/vendor/tinymemory/crates/tinymemory-tools/src/recall"]
//! [evaluation]
//! seeds = [27, 28]
//! [budget]
//! usd_cap = 10.0
//! max_epochs = 3
//! proposals_per_epoch = 2
//! "#;
//! let campaign = Campaign::parse(manifest, Path::new("recall-pack.toml"))?;
//! assert_eq!(campaign.metric.direction, Direction::Maximize);
//! assert_eq!(campaign.frozen_paths(), ["benchmarks"]);
//! # Ok::<(), ohrsi::Error>(())
//! ```
//!
//! ## What this crate does not hold
//!
//! It does not run the loop yet: wiring tinyrsi's script loop to a campaign is
//! the next phase of `docs/plans/rsi-memory-loop.md`. It never holds benchmark
//! code or data — those stay in the `benchmarks/` submodule, frozen, so a
//! candidate cannot improve its score by changing how it is measured.

pub mod campaign;
pub mod cli;
pub mod error;
pub mod workspace;

pub use campaign::{Benchmark, Budget, Campaign, Direction, Evaluation, Metric, Surface};
pub use error::{Error, Result};
pub use workspace::{SubmoduleStatus, Workspace};
