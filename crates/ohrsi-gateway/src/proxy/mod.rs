//! The OpenAI-compatible HTTP surface and the forwarding pipeline.
//!
//! Every call goes through the same steps: prepare (validate, pin, bound),
//! reserve against both ledgers, forward to the upstream, read the actual
//! usage (scanning the stream as it passes through), settle, and meter.
//!
//! Routes:
//!
//! - `POST /v1/chat/completions`, `POST /v1/embeddings`
//! - the same under `/__tag/{phase}/{seat}/v1/...`, for clients that cannot
//!   set `x-ohrsi-*` headers (CortexDB, the benchmark harness)
//! - `GET /v1/models` — the allowlist
//! - `GET /status` — both ledgers

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use futures_util::StreamExt;
use serde_json::{Value, json};

use crate::error::Error;
use crate::ledger::{Budgets, Reservation};
use crate::meter::{Meter, MeterRow, Tags};
use crate::price::PriceTable;
use crate::request::{Endpoint, Limits, Prepared, prepare};
use crate::usage::{SseUsage, Usage};

/// Static configuration for a gateway.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// Upstream base URL, such as `https://openrouter.ai/api/v1`.
    pub upstream: String,
    /// The real provider key; clients never see it.
    pub api_key: String,
    /// The model allowlist and price ceilings.
    pub prices: PriceTable,
    /// Completion-length limits.
    pub limits: Limits,
}

/// A running gateway's shared state.
#[derive(Debug)]
pub struct Gateway {
    config: GatewayConfig,
    budgets: Mutex<Budgets>,
    meter: Meter,
    client: reqwest::Client,
}

impl Gateway {
    /// Builds a gateway over `budgets`, metering to `meter`.
    #[must_use]
    pub fn new(config: GatewayConfig, budgets: Budgets, meter: Meter) -> Arc<Self> {
        Arc::new(Self {
            config,
            budgets: Mutex::new(budgets),
            meter,
            client: reqwest::Client::new(),
        })
    }

    /// The axum router serving every gateway route.
    pub fn router(self: Arc<Self>) -> Router {
        Router::new()
            .route("/v1/chat/completions", post(chat))
            .route("/v1/embeddings", post(embeddings))
            .route(
                "/__tag/{phase}/{seat}/v1/chat/completions",
                post(tagged_chat),
            )
            .route(
                "/__tag/{phase}/{seat}/v1/embeddings",
                post(tagged_embeddings),
            )
            .route("/v1/models", get(models))
            .route("/status", get(status))
            // Long agent sessions resend their whole history every call.
            .layer(axum::extract::DefaultBodyLimit::max(64 * 1024 * 1024))
            .with_state(self)
    }

    /// Both ledgers as JSON: `{"run": .., "total": ..}`.
    #[must_use]
    pub fn status(&self) -> Value {
        let budgets = self.budgets.lock().unwrap_or_else(PoisonError::into_inner);
        let (run, total) = budgets.states();
        json!({ "run": run, "total": total })
    }

    fn reserve(&self, amount: f64) -> Result<Reservation, Error> {
        self.budgets
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .reserve(amount)
    }

    fn settle(&self, reservation: Reservation, cost: Option<f64>) -> Option<String> {
        self.budgets
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .settle(reservation, cost)
            .err()
            .map(|error| error.to_string())
    }

    fn record(&self, row: &MeterRow) {
        // A meter write failure must not fail the call the ledger already
        // charged; it is reported on stderr instead.
        if let Err(error) = self.meter.record(row) {
            eprintln!("ohrsi-gateway: {error}");
        }
    }
}

type Shared = State<Arc<Gateway>>;

async fn chat(State(gw): Shared, headers: HeaderMap, body: Bytes) -> Response {
    forward(gw, Endpoint::Chat, header_tags(&headers), body).await
}

async fn embeddings(State(gw): Shared, headers: HeaderMap, body: Bytes) -> Response {
    forward(gw, Endpoint::Embeddings, header_tags(&headers), body).await
}

async fn tagged_chat(
    State(gw): Shared,
    Path((phase, seat)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    forward(gw, Endpoint::Chat, path_tags(&headers, phase, seat), body).await
}

async fn tagged_embeddings(
    State(gw): Shared,
    Path((phase, seat)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    forward(
        gw,
        Endpoint::Embeddings,
        path_tags(&headers, phase, seat),
        body,
    )
    .await
}

async fn models(State(gw): Shared) -> Response {
    let data: Vec<Value> = gw
        .config
        .prices
        .models()
        .map(|id| json!({ "id": id, "object": "model" }))
        .collect();
    axum::Json(json!({ "object": "list", "data": data })).into_response()
}

async fn status(State(gw): Shared) -> Response {
    axum::Json(gw.status()).into_response()
}

fn header_tags(headers: &HeaderMap) -> Tags {
    let get = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_owned()
    };
    Tags {
        phase: get("x-ohrsi-phase"),
        seat: get("x-ohrsi-seat"),
        candidate: get("x-ohrsi-candidate"),
    }
}

fn path_tags(headers: &HeaderMap, phase: String, seat: String) -> Tags {
    Tags {
        phase,
        seat,
        ..header_tags(headers)
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

fn error_status(error: &Error) -> StatusCode {
    match error {
        Error::BudgetExceeded { .. } | Error::PriceBoundBreached => StatusCode::PAYMENT_REQUIRED,
        Error::UnknownModel(_) => StatusCode::FORBIDDEN,
        Error::Upstream(_) => StatusCode::BAD_GATEWAY,
        Error::BadRequest(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (
        status,
        axum::Json(json!({ "error": { "message": message, "code": status.as_u16() } })),
    )
        .into_response()
}

/// The outcome of one call, before it is metered.
struct Outcome {
    status: u16,
    stream: bool,
    reserved: f64,
    usage: Usage,
    cost: Option<f64>,
    error: Option<String>,
}

async fn forward(gw: Arc<Gateway>, endpoint: Endpoint, tags: Tags, body: Bytes) -> Response {
    let started = Instant::now();
    let row = move |model: &str, outcome: Outcome| MeterRow {
        at_ms: now_ms(),
        tags: tags.clone(),
        model: model.to_owned(),
        endpoint: endpoint.path().to_owned(),
        status: outcome.status,
        stream: outcome.stream,
        reserved_usd: outcome.reserved,
        cost_usd: outcome.cost,
        prompt_tokens: outcome.usage.prompt_tokens,
        completion_tokens: outcome.usage.completion_tokens,
        wall_ms: started.elapsed().as_millis(),
        error: outcome.error,
    };
    let refuse = |model: &str, error: &Error| {
        let status = error_status(error);
        gw.record(&row(
            model,
            Outcome {
                status: status.as_u16(),
                stream: false,
                reserved: 0.0,
                usage: Usage::default(),
                cost: None,
                error: Some(error.to_string()),
            },
        ));
        error_response(status, &error.to_string())
    };
    let parsed = serde_json::from_slice::<Value>(&body)
        .map_err(|error| Error::BadRequest(format!("invalid JSON: {error}")))
        .and_then(|value| prepare(endpoint, value, &gw.config.prices, gw.config.limits));
    let prepared = match parsed {
        Ok(prepared) => prepared,
        Err(error) => return refuse("", &error),
    };
    let reservation = match gw.reserve(prepared.reserve_usd) {
        Ok(reservation) => reservation,
        Err(error) => return refuse(&prepared.model, &error),
    };
    let url = format!(
        "{}/{}",
        gw.config.upstream.trim_end_matches('/'),
        endpoint.path()
    );
    let sent = gw
        .client
        .post(url)
        .bearer_auth(&gw.config.api_key)
        .json(&prepared.body)
        .send()
        .await;
    let response = match sent {
        Ok(response) => response,
        Err(error) => {
            let error = Error::Upstream(error.to_string());
            let settle_error = gw.settle(reservation, None);
            gw.record(&row(
                &prepared.model,
                Outcome {
                    status: StatusCode::BAD_GATEWAY.as_u16(),
                    stream: prepared.stream,
                    reserved: reservation.amount(),
                    usage: Usage::default(),
                    cost: None,
                    error: Some(settle_error.unwrap_or_else(|| error.to_string())),
                },
            ));
            return error_response(StatusCode::BAD_GATEWAY, &error.to_string());
        }
    };
    if prepared.stream && response.status().is_success() {
        return stream_through(
            gw.clone(),
            prepared,
            reservation,
            response,
            move |outcome| row(&outcome.0, outcome.1),
        );
    }
    let model = prepared.model.clone();
    let (response, outcome) = buffered(&gw, &prepared, reservation, response).await;
    gw.record(&row(&model, outcome));
    response
}

/// Reads a non-streaming (or failed) response whole, then settles it.
async fn buffered(
    gw: &Gateway,
    prepared: &Prepared,
    reservation: Reservation,
    response: reqwest::Response,
) -> (Response, Outcome) {
    let status = response.status();
    let bytes = response.bytes().await.unwrap_or_default();
    let usage = serde_json::from_slice::<Value>(&bytes)
        .map_or_else(|_| Usage::default(), |v| Usage::from_body(&v));
    let price = gw.config.prices.get(&prepared.model).ok();
    let cost = if status.is_success() {
        price.and_then(|p| usage.cost_or_ceiling(p))
    } else if status.is_client_error() && status != StatusCode::REQUEST_TIMEOUT {
        // OpenRouter does not bill a request it rejects outright.
        Some(usage.cost.unwrap_or(0.0))
    } else {
        usage.cost
    };
    let settle_error = gw.settle(reservation, cost);
    let failure =
        (!status.is_success()).then(|| String::from_utf8_lossy(&bytes).chars().take(500).collect());
    let outcome = Outcome {
        status: status.as_u16(),
        stream: false,
        reserved: reservation.amount(),
        usage,
        cost,
        error: settle_error.or(failure),
    };
    let response = Response::builder()
        .status(status.as_u16())
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(bytes))
        .unwrap_or_else(|_| {
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "response build failed")
        });
    (response, outcome)
}

fn stream_through<F>(
    gw: Arc<Gateway>,
    prepared: Prepared,
    reservation: Reservation,
    response: reqwest::Response,
    row: F,
) -> Response
where
    F: FnOnce((String, Outcome)) -> MeterRow + Send + 'static,
{
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(64);
    tokio::spawn(async move {
        let mut scanner = SseUsage::default();
        let mut upstream = response.bytes_stream();
        let mut failure = None;
        while let Some(chunk) = upstream.next().await {
            match chunk {
                Ok(chunk) => {
                    scanner.feed(&chunk);
                    // Keep draining after the client goes away: the usage
                    // event arrives last and the call is billed regardless.
                    let _ = tx.send(Ok(chunk)).await;
                }
                Err(error) => {
                    failure = Some(error.to_string());
                    let _ = tx.send(Err(std::io::Error::other(error.to_string()))).await;
                    break;
                }
            }
        }
        drop(tx);
        let usage = scanner.finish();
        let cost = gw
            .config
            .prices
            .get(&prepared.model)
            .ok()
            .and_then(|price| usage.cost_or_ceiling(price));
        let settle_error = gw.settle(reservation, cost);
        gw.record(&row((
            prepared.model.clone(),
            Outcome {
                status: 200,
                stream: true,
                reserved: reservation.amount(),
                usage,
                cost,
                error: settle_error.or(failure),
            },
        )));
    });
    let body = futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    });
    Response::builder()
        .status(200)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .body(Body::from_stream(body))
        .unwrap_or_else(|_| {
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "response build failed")
        })
}

#[cfg(test)]
mod test;
