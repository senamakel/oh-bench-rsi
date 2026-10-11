//! Unit tests for the ledger lock.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::LedgerLock;
use crate::error::Error;

#[test]
fn excludes_a_second_owner_until_released() {
    let dir = std::env::temp_dir().join(format!("ohrsi-gw-{}-lock", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let ledger = dir.join("ledger-total.json");
    let first = LedgerLock::acquire(&ledger).unwrap();
    assert!(matches!(
        LedgerLock::acquire(&ledger).unwrap_err(),
        Error::Locked(_)
    ));
    drop(first);
    let _again = LedgerLock::acquire(&ledger).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reports_unusable_lock_paths() {
    let file = std::env::temp_dir().join(format!("ohrsi-gw-{}-lock-file", std::process::id()));
    std::fs::write(&file, "").unwrap();
    let error = LedgerLock::acquire(&file.join("ledger.json")).unwrap_err();
    assert!(matches!(error, Error::Io { .. }), "{error}");
    let _ = std::fs::remove_file(&file);
}
