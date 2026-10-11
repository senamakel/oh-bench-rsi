//! Campaign manifests: what to improve, how to measure it, and what may change.
//!
//! A campaign is the unit of work for the improvement loop. [`Campaign::parse`]
//! and [`Campaign::load`] turn a TOML manifest into a [`Campaign`] and reject
//! any manifest that would let the loop win by moving the goalposts — for
//! example an editable path inside the frozen `benchmarks/` checkout.

mod types;

use std::path::Path;

pub use types::{Benchmark, Budget, Campaign, Direction, Evaluation, Metric, Surface};

use crate::error::{Error, Result};

/// The vendored OpenHuman checkout every editable path must live under.
pub const OPENHUMAN_ROOT: &str = "vendor/openhuman";

/// Paths that are frozen in every campaign, whether or not it lists them.
pub const ALWAYS_FROZEN: &[&str] = &["benchmarks"];

impl Campaign {
    /// Reads and validates the manifest at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the file cannot be read, [`Error::Parse`] if it
    /// is not a valid manifest, or any validation error from
    /// [`Campaign::validate`].
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&text, path)
    }

    /// Parses and validates manifest `text`; `origin` names it in errors.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Parse`] if `text` is not a valid manifest, or any
    /// validation error from [`Campaign::validate`].
    pub fn parse(text: &str, origin: &Path) -> Result<Self> {
        let campaign: Self = toml::from_str(text).map_err(|error| Error::Parse {
            path: origin.to_path_buf(),
            message: error.message().to_owned(),
        })?;
        campaign.validate()?;
        Ok(campaign)
    }

    /// Checks every rule a manifest must satisfy beyond its shape.
    ///
    /// # Errors
    ///
    /// Returns the first rule the campaign breaks: an invalid name, an empty
    /// command or metric name, a non-finite target, an unsafe or misplaced
    /// path, an editable path overlapping a frozen one, too few or duplicate
    /// seeds, or a zero or non-finite budget.
    pub fn validate(&self) -> Result<()> {
        if !is_kebab_case(&self.name) {
            return Err(Error::InvalidName(self.name.clone()));
        }
        if self.benchmark.command.first().is_none_or(String::is_empty) {
            return Err(Error::EmptyCommand);
        }
        // `.` names the repository root, the one place a lone `.` is allowed.
        if self.benchmark.workdir != "." {
            check_relative(&self.benchmark.workdir)?;
        }
        if self.metric.name.trim().is_empty() {
            return Err(Error::EmptyMetricName);
        }
        if self.metric.target.is_some_and(|target| !target.is_finite()) {
            return Err(Error::NonFiniteTarget);
        }
        self.validate_surface()?;
        validate_seeds(&self.evaluation.seeds)?;
        self.validate_budget()
    }

    /// Every frozen path: the campaign's own list plus [`ALWAYS_FROZEN`].
    #[must_use]
    pub fn frozen_paths(&self) -> Vec<&str> {
        let mut frozen: Vec<&str> = ALWAYS_FROZEN.to_vec();
        for path in &self.surface.frozen {
            if !frozen.contains(&path.as_str()) {
                frozen.push(path);
            }
        }
        frozen
    }

    fn validate_surface(&self) -> Result<()> {
        if self.surface.editable.is_empty() {
            return Err(Error::NoEditable);
        }
        for path in &self.surface.frozen {
            check_relative(path)?;
        }
        let frozen = self.frozen_paths();
        for path in &self.surface.editable {
            check_relative(path)?;
            if !is_within(path, OPENHUMAN_ROOT) || path.trim_end_matches('/') == OPENHUMAN_ROOT {
                return Err(Error::OutsideOpenHuman(path.clone()));
            }
            if let Some(hit) = frozen
                .iter()
                .find(|f| is_within(path, f) || is_within(f, path))
            {
                return Err(Error::FrozenEditable {
                    path: path.clone(),
                    frozen: (*hit).to_owned(),
                });
            }
        }
        Ok(())
    }

    fn validate_budget(&self) -> Result<()> {
        let cap = self.budget.usd_cap;
        if !cap.is_finite() || cap <= 0.0 {
            return Err(Error::InvalidSpendCap);
        }
        if self.budget.max_epochs == 0 {
            return Err(Error::ZeroLimit("max_epochs"));
        }
        if self.budget.proposals_per_epoch == 0 {
            return Err(Error::ZeroLimit("proposals_per_epoch"));
        }
        Ok(())
    }
}

fn is_kebab_case(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn check_relative(path: &str) -> Result<()> {
    let unsafe_path = path.trim().is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.split('/').any(|part| part == ".." || part == ".");
    if unsafe_path {
        return Err(Error::UnsafePath(path.to_owned()));
    }
    Ok(())
}

/// Whether `path` is `base` or lies beneath it, compared by whole components.
fn is_within(path: &str, base: &str) -> bool {
    let path = path.trim_end_matches('/');
    let base = base.trim_end_matches('/');
    path == base
        || path
            .strip_prefix(base)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn validate_seeds(seeds: &[u64]) -> Result<()> {
    if seeds.len() < 2 {
        return Err(Error::TooFewSeeds(seeds.len()));
    }
    for (index, seed) in seeds.iter().enumerate() {
        if seeds[..index].contains(seed) {
            return Err(Error::DuplicateSeed(*seed));
        }
    }
    Ok(())
}

#[cfg(test)]
mod test;
