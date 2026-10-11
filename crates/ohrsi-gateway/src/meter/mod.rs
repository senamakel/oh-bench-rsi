//! The per-call meter: one JSON line per call admitted or refused.
//!
//! Rows carry the caller's tags (`phase`, `seat`, `candidate`), so a run's
//! spend can be broken down by swarm seat or benchmark phase afterwards.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{Error, Result};

/// Who made a call, from `x-ohrsi-*` headers or a `/__tag/` path prefix.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Tags {
    /// Benchmark or loop phase, such as `ingest`, `reader`, `judge`, `swarm`.
    pub phase: String,
    /// The swarm seat, if any.
    pub seat: String,
    /// The candidate being built or measured, if any.
    pub candidate: String,
}

/// One metered call.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MeterRow {
    /// Milliseconds since the Unix epoch.
    pub at_ms: u128,
    /// Caller tags.
    #[serde(flatten)]
    pub tags: Tags,
    /// Model id.
    pub model: String,
    /// `chat/completions` or `embeddings`.
    pub endpoint: String,
    /// HTTP status returned to the caller.
    pub status: u16,
    /// Whether the response streamed.
    pub stream: bool,
    /// Dollars reserved.
    pub reserved_usd: f64,
    /// Dollars charged on settlement, if known.
    pub cost_usd: Option<f64>,
    /// Prompt tokens.
    pub prompt_tokens: u64,
    /// Completion tokens.
    pub completion_tokens: u64,
    /// Wall-clock duration.
    pub wall_ms: u128,
    /// Refusal or failure reason.
    pub error: Option<String>,
}

/// An append-only JSONL file.
#[derive(Debug)]
pub struct Meter {
    path: PathBuf,
}

impl Meter {
    /// Uses `path`, creating its directory.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the directory cannot be created.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    /// Appends one row.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the file cannot be written.
    pub fn record(&self, row: &MeterRow) -> Result<()> {
        let io = |source| Error::Io {
            path: self.path.clone(),
            source,
        };
        let mut line = serde_json::to_vec(row).map_err(|error| io(std::io::Error::other(error)))?;
        line.push(b'\n');
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .and_then(|mut file| file.write_all(&line))
            .map_err(io)
    }
}

#[cfg(test)]
mod test;
