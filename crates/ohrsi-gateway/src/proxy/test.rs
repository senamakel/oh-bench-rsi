//! Unit tests for tag extraction and error mapping.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use axum::http::{HeaderMap, HeaderValue, StatusCode};

use super::{error_status, header_tags, path_tags};
use crate::error::Error;

#[test]
fn reads_tags_from_headers_and_paths() {
    let mut headers = HeaderMap::new();
    headers.insert("x-ohrsi-phase", HeaderValue::from_static("swarm"));
    headers.insert("x-ohrsi-seat", HeaderValue::from_static("lead"));
    headers.insert("x-ohrsi-candidate", HeaderValue::from_static("c1"));
    let tags = header_tags(&headers);
    assert_eq!(
        (
            tags.phase.as_str(),
            tags.seat.as_str(),
            tags.candidate.as_str()
        ),
        ("swarm", "lead", "c1")
    );
    let tags = path_tags(&headers, "ingest".into(), "cortex".into());
    assert_eq!(
        (
            tags.phase.as_str(),
            tags.seat.as_str(),
            tags.candidate.as_str()
        ),
        ("ingest", "cortex", "c1")
    );
    assert_eq!(header_tags(&HeaderMap::new()).phase, "");
}

#[test]
fn maps_errors_to_statuses() {
    let budget = Error::BudgetExceeded {
        ledger: "run",
        needed: 1.0,
        remaining: 0.0,
    };
    assert_eq!(error_status(&budget), StatusCode::PAYMENT_REQUIRED);
    assert_eq!(
        error_status(&Error::PriceBoundBreached),
        StatusCode::PAYMENT_REQUIRED
    );
    assert_eq!(
        error_status(&Error::UnknownModel("x".into())),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        error_status(&Error::Upstream("x".into())),
        StatusCode::BAD_GATEWAY
    );
    assert_eq!(
        error_status(&Error::BadRequest("x".into())),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        error_status(&Error::Usage("x".into())),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}
