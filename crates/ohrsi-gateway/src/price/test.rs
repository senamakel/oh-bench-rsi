//! Unit tests for the price table.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::{EMBEDDING, GLM_FLASH, PriceTable};
use crate::error::Error;

#[test]
fn allows_exactly_the_swarm_and_embedding_models() {
    let table = PriceTable::standard();
    assert_eq!(table.models().collect::<Vec<_>>(), [EMBEDDING, GLM_FLASH]);
    assert_eq!(table.get(GLM_FLASH).unwrap().provider, "z-ai");
}

#[test]
fn refuses_unknown_models() {
    let error = PriceTable::standard().get("openai/gpt-5.2").unwrap_err();
    assert!(matches!(error, Error::UnknownModel(_)));
}
