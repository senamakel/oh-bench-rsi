//! Loading a hive package from disk into the language library.
//!
//! The language opens no files: the host supplies every package file as a
//! relative path → text map. This module walks the package directory, keeps
//! the files the language reads (`*.json` manifests and `*.md` documents,
//! excluding directory `README.md` indexes), and returns the validated,
//! lowered hive.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Context;
use tinyhivemind_lang::LoweredHive;

/// Reads, parses, validates and lowers the package rooted at `dir`.
pub fn load(dir: &Path) -> anyhow::Result<LoweredHive> {
    let files = read_files(dir)?;
    let package = tinyhivemind_lang::parse(&files).context("parse hive package")?;
    tinyhivemind_lang::validate(&package).context("validate hive package")?;
    tinyhivemind_lang::lower(&package).context("lower hive package")
}

fn read_files(dir: &Path) -> anyhow::Result<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).with_context(|| format!("read {}", next.display()))? {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let rel = path.strip_prefix(dir)?.to_string_lossy().replace('\\', "/");
            let keep = rel.ends_with(".json") || (rel.ends_with(".md") && rel.contains('/') && !rel.ends_with("README.md"));
            if keep {
                files.insert(rel, std::fs::read_to_string(&path)?);
            }
        }
    }
    Ok(files)
}
