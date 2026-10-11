//! Crate-wide error type.
//!
//! Every fallible public function in `ohrsi` returns [`Result<T>`]. Each
//! validation rule a campaign manifest can break has its own variant, so a
//! caller (and a test) can tell exactly which rule failed without parsing a
//! message.

use std::path::PathBuf;

/// Everything that can go wrong while loading a workspace or a campaign.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A file or directory could not be read.
    #[error("cannot read {path}: {source}")]
    Io {
        /// The path that failed.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// A campaign manifest is not valid TOML or does not match the schema.
    #[error("cannot parse {path}: {message}")]
    Parse {
        /// The manifest that failed to parse.
        path: PathBuf,
        /// The parser's description of the problem.
        message: String,
    },
    /// No directory at or above the start path looks like this repository.
    #[error(
        "no oh-bench-rsi workspace (a directory with Cargo.toml and campaigns/) at or above {start}"
    )]
    WorkspaceNotFound {
        /// Where the search started.
        start: PathBuf,
    },
    /// A campaign name is empty or not lowercase kebab-case.
    #[error("campaign name {0:?} must be non-empty lowercase kebab-case")]
    InvalidName(String),
    /// The benchmark command has no program.
    #[error("benchmark command must name a program")]
    EmptyCommand,
    /// The metric has no name.
    #[error("metric name must not be empty")]
    EmptyMetricName,
    /// The metric target is NaN or infinite.
    #[error("metric target must be a finite number")]
    NonFiniteTarget,
    /// A path is absolute, empty, or escapes the repository with `..`.
    #[error("path {0:?} must be repository-relative without `..`")]
    UnsafePath(String),
    /// The campaign lists no editable paths.
    #[error("campaign must list at least one editable path")]
    NoEditable,
    /// An editable path lies outside the vendored OpenHuman checkout.
    #[error("editable path {0:?} must lie under vendor/openhuman/")]
    OutsideOpenHuman(String),
    /// An editable path overlaps a frozen path, such as the benchmarks.
    #[error("editable path {path:?} overlaps frozen path {frozen:?}")]
    FrozenEditable {
        /// The offending editable path.
        path: String,
        /// The frozen path it overlaps.
        frozen: String,
    },
    /// Fewer than two seeds, so no paired comparison is possible.
    #[error("campaign needs at least two seeds, found {0}")]
    TooFewSeeds(usize),
    /// The same seed appears twice.
    #[error("seed {0} is listed more than once")]
    DuplicateSeed(u64),
    /// The spend cap is zero, negative, or not finite.
    #[error("budget usd_cap must be a positive finite number")]
    InvalidSpendCap,
    /// The epoch or proposal limit is zero.
    #[error("budget {0} must be at least 1")]
    ZeroLimit(&'static str),
}

/// Result alias used by every fallible public function in this crate.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod test;
