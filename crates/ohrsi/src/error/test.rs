//! Unit tests for the crate-wide error type.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::Error;

#[test]
fn messages_are_lowercase_without_trailing_punctuation() {
    let errors = [
        Error::EmptyCommand,
        Error::NoEditable,
        Error::TooFewSeeds(1),
        Error::ZeroLimit("max_epochs"),
        Error::OutsideOpenHuman("benchmarks/memory".into()),
    ];
    for error in errors {
        let message = error.to_string();
        let first = message.chars().next().expect("messages are never empty");
        assert!(first.is_lowercase(), "{message}");
        assert!(!message.ends_with('.'), "{message}");
    }
}

#[test]
fn io_errors_keep_their_source() {
    let error = Error::Io {
        path: "campaigns".into(),
        source: std::io::Error::other("denied"),
    };
    assert!(std::error::Error::source(&error).is_some());
    assert_eq!(error.to_string(), "cannot read campaigns: denied");
}
