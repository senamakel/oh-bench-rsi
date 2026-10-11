//! The campaign manifest schema, one struct per TOML table.

use serde::Deserialize;

/// One improvement campaign: a benchmark to move, the OpenHuman code a
/// proposal may change, and the limits the loop must respect.
///
/// A campaign lives in `campaigns/<name>.toml`. See `docs/specs/campaigns.md`
/// for the full contract.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Campaign {
    /// Lowercase kebab-case identifier; also the run directory prefix.
    pub name: String,
    /// What the campaign is trying to improve, shown to the proposer verbatim.
    pub brief: String,
    /// How to run the benchmark that measures a candidate.
    pub benchmark: Benchmark,
    /// The number the loop optimizes.
    pub metric: Metric,
    /// Which paths a proposal may and may not touch.
    pub surface: Surface,
    /// Seeds shared by the incumbent and every candidate.
    pub evaluation: Evaluation,
    /// Hard limits on spend and iteration.
    pub budget: Budget,
}

/// The command that measures one candidate on one seed.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Benchmark {
    /// Repository-relative directory the command runs in.
    pub workdir: String,
    /// Program followed by its arguments. The command must print
    /// `METRIC <name>=<value>` on stdout and read its seed from
    /// `TINYRSI_SEED`.
    pub command: Vec<String>,
    /// Environment variable names passed through to the command, such as
    /// `OPENROUTER_API_KEY`. Everything else is cleared.
    #[serde(default)]
    pub env: Vec<String>,
    /// Wall-clock limit for one seed, in seconds.
    pub timeout_secs: u64,
}

/// Whether a larger metric value is better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Larger is better, such as accuracy.
    Maximize,
    /// Smaller is better, such as latency.
    Minimize,
}

/// The metric the benchmark command prints.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metric {
    /// Name in the `METRIC <name>=<value>` line.
    pub name: String,
    /// Whether to maximize or minimize it.
    pub direction: Direction,
    /// Optional value at which the campaign stops early.
    pub target: Option<f64>,
}

/// The paths a proposal may edit, and the paths it must never edit.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Surface {
    /// Repository-relative files or directories under `vendor/openhuman/`.
    pub editable: Vec<String>,
    /// Additional repository-relative paths that must stay untouched.
    /// `benchmarks` is always frozen whether or not it is listed.
    #[serde(default)]
    pub frozen: Vec<String>,
}

/// The seeds every arm runs on, so comparisons are paired.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evaluation {
    /// At least two distinct seeds.
    pub seeds: Vec<u64>,
}

/// Hard limits for one campaign run.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    /// Maximum provider spend, in US dollars, across every model call.
    pub usd_cap: f64,
    /// Maximum improvement epochs.
    pub max_epochs: u32,
    /// Proposals generated per epoch.
    pub proposals_per_epoch: u32,
}
