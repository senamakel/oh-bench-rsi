//! The repository on disk: its root, its pinned checkouts, and its campaigns.
//!
//! [`Workspace::discover`] walks up from a directory to the repository root,
//! the first ancestor holding both `Cargo.toml` and `campaigns/`. From there it
//! reports whether each pinned submodule is populated and lists the campaign
//! manifests, so `ohrsi check` can tell a broken manifest from a checkout that
//! has simply not been bootstrapped.

use std::path::{Path, PathBuf};

use crate::campaign::Campaign;
use crate::error::{Error, Result};

/// The submodules this repository pins, relative to its root.
pub const SUBMODULES: &[&str] = &["vendor/openhuman", "vendor/tinyrsi", "benchmarks"];

/// The directory campaign manifests live in, relative to the root.
pub const CAMPAIGNS_DIR: &str = "campaigns";

/// Whether one pinned submodule has been checked out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmoduleStatus {
    /// Repository-relative path of the submodule.
    pub path: &'static str,
    /// True when the directory exists and is not empty.
    pub populated: bool,
}

/// A located oh-bench-rsi checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    /// Uses `root` as the repository root without searching.
    #[must_use]
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Finds the repository root at or above `start`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::WorkspaceNotFound`] if no ancestor of `start` holds
    /// both `Cargo.toml` and a `campaigns/` directory.
    pub fn discover(start: &Path) -> Result<Self> {
        start
            .ancestors()
            .find(|dir| dir.join("Cargo.toml").is_file() && dir.join(CAMPAIGNS_DIR).is_dir())
            .map(Self::at)
            .ok_or_else(|| Error::WorkspaceNotFound {
                start: start.to_path_buf(),
            })
    }

    /// The repository root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Reports whether each pinned submodule is checked out.
    #[must_use]
    pub fn submodules(&self) -> Vec<SubmoduleStatus> {
        SUBMODULES
            .iter()
            .map(|path| SubmoduleStatus {
                path,
                populated: std::fs::read_dir(self.root.join(path))
                    .is_ok_and(|mut entries| entries.next().is_some()),
            })
            .collect()
    }

    /// Lists every `*.toml` manifest in `campaigns/`, sorted by path.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the directory cannot be read.
    pub fn campaign_paths(&self) -> Result<Vec<PathBuf>> {
        let dir = self.root.join(CAMPAIGNS_DIR);
        let io = |source| Error::Io {
            path: dir.clone(),
            source,
        };
        let mut paths = Vec::new();
        for entry in std::fs::read_dir(&dir).map_err(io)? {
            let path = entry.map_err(io)?.path();
            if path.extension().is_some_and(|ext| ext == "toml") {
                paths.push(path);
            }
        }
        paths.sort();
        Ok(paths)
    }

    /// Editable and frozen paths of `campaign` that do not exist on disk.
    ///
    /// A missing path is not a manifest error on its own: it usually means a
    /// submodule has not been initialized, or that upstream moved the file.
    #[must_use]
    pub fn missing_paths<'a>(&self, campaign: &'a Campaign) -> Vec<&'a str> {
        campaign
            .surface
            .editable
            .iter()
            .chain(&campaign.surface.frozen)
            .map(String::as_str)
            .filter(|path| !self.root.join(path).exists())
            .collect()
    }
}

#[cfg(test)]
mod test;
