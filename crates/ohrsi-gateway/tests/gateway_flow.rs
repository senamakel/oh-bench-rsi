//! End-to-end: a real gateway in front of a mock OpenRouter.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use ohrsi_gateway::{Bound, Options};
use serde_json::{Value, json};

type Seen = Arc<Mutex<Vec<(String, Value)>>>;

async fn mock_chat(
    State(seen): State<Seen>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    seen.lock().unwrap().push((auth, body.clone()));
    let text = body["messages"][0]["content"].as_str().unwrap_or("");
    match text {
        "reject" => (axum::http::StatusCode::BAD_REQUEST, Json(json!({"error": "bad"}))).into_response(),
        "crash" => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "oops").into_response(),
        "overcost" => Json(json!({"choices": [], "usage": {"cost": 10.0}})).into_response(),
        "slow" => {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            Json(json!({"choices": [], "usage": {"cost": 0.003}})).into_response()
        }
        "tokens" => Json(json!({"choices": [], "usage": {"prompt_tokens": 1000, "completion_tokens": 1000}})).into_response(),
        _ if body["stream"] == json!(true) => {
            let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\ndata: {\"choices\":[],\"usage\":{\"cost\":0.001,\"prompt_tokens\":5,\"completion_tokens\":1}}\n\ndata: [DONE]\n\n";
            ([("content-type", "text/event-stream")], sse).into_response()
        }
        _ => Json(json!({"choices": [{"message": {"content": "ok"}}], "usage": {"cost": 0.002, "prompt_tokens": 5, "completion_tokens": 1}})).into_response(),
    }
}

async fn mock_embeddings(Json(_body): Json<Value>) -> Response {
    Json(json!({"data": [{"embedding": [0.0]}], "usage": {"cost": 0.00001, "prompt_tokens": 2}}))
        .into_response()
}

struct Harness {
    dir: PathBuf,
    base: String,
    seen: Seen,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    client: reqwest::Client,
}

impl Harness {
    async fn start(name: &str, run_limit: f64, total_limit: f64) -> Self {
        let dir = std::env::temp_dir().join(format!("ohrsi-gw-flow-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let seen: Seen = Arc::default();
        let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream.local_addr().unwrap();
        let app = axum::Router::new()
            .route("/v1/chat/completions", post(mock_chat))
            .route("/v1/embeddings", post(mock_embeddings))
            .with_state(seen.clone());
        tokio::spawn(async move { axum::serve(upstream, app).await.unwrap() });
        let options = Options {
            run_dir: dir.join("run-1"),
            ledger_total: dir.join("ledger-total.json"),
            run_limit,
            total_limit,
            listen: "127.0.0.1:0".parse().unwrap(),
            upstream: format!("http://{upstream_addr}/v1"),
            capture: true,
        };
        let bound = Bound::open(&options, "real-key".into()).await.unwrap();
        let base = format!("http://{}", bound.addr);
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            bound
                .serve(async {
                    let _ = rx.await;
                })
                .await
                .unwrap();
        });
        Self {
            dir,
            base,
            seen,
            stop: Some(tx),
            client: reqwest::Client::new(),
        }
    }

    async fn chat(&self, path: &str, content: &str, extra: Value) -> (u16, String) {
        let mut body = json!({"model": "z-ai/glm-5.3-flash", "messages": [{"role": "user", "content": content}], "max_tokens": 100});
        for (k, v) in extra.as_object().unwrap() {
            body[k] = v.clone();
        }
        let response = self
            .client
            .post(format!("{}{path}", self.base))
            .bearer_auth("dummy")
            .header("x-ohrsi-candidate", "c1")
            .json(&body)
            .send()
            .await
            .unwrap();
        (response.status().as_u16(), response.text().await.unwrap())
    }

    async fn status(&self) -> Value {
        self.client
            .get(format!("{}/status", self.base))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    fn meter(&self) -> Vec<Value> {
        let text = std::fs::read_to_string(self.dir.join("run-1/meter.jsonl")).unwrap_or_default();
        text.lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[tokio::test]
async fn forwards_with_the_real_key_and_charges_reported_cost() {
    let h = Harness::start("charge", 5.0, 50.0).await;
    let (status, _) = h
        .chat(
            "/__tag/reader/harness/v1/chat/completions",
            "hello",
            json!({}),
        )
        .await;
    assert_eq!(status, 200);
    let seen = h.seen.lock().unwrap().clone();
    assert_eq!(seen[0].0, "Bearer real-key");
    assert_eq!(seen[0].1["provider"]["only"], json!(["z-ai"]));
    let ledgers = h.status().await;
    assert!((ledgers["run"]["settled_usd"].as_f64().unwrap() - 0.002).abs() < 1e-12);
    assert!((ledgers["total"]["settled_usd"].as_f64().unwrap() - 0.002).abs() < 1e-12);
    let rows = h.meter();
    assert_eq!(rows[0]["phase"], "reader");
    assert_eq!(rows[0]["seat"], "harness");
    assert_eq!(rows[0]["candidate"], "c1");
    let captured = std::fs::read_dir(h.dir.join("run-1/captures"))
        .unwrap()
        .count();
    assert_eq!(captured, 1);
}

#[tokio::test]
async fn streams_through_and_settles_from_the_usage_event() {
    let h = Harness::start("stream", 5.0, 50.0).await;
    let (status, text) = h
        .chat("/v1/chat/completions", "hello", json!({"stream": true}))
        .await;
    assert_eq!(status, 200);
    assert!(text.contains("[DONE]"));
    // Settlement happens after the last byte; give the task a moment.
    for _ in 0..50 {
        if h.meter().len() == 1 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let ledgers = h.status().await;
    assert!((ledgers["run"]["settled_usd"].as_f64().unwrap() - 0.001).abs() < 1e-12);
    assert_eq!(h.meter()[0]["stream"], true);
}

#[tokio::test]
async fn refuses_calls_beyond_the_run_cap_with_402() {
    let h = Harness::start("cap", 0.01, 50.0).await;
    let (status, text) = h
        .chat(
            "/v1/chat/completions",
            "hello",
            json!({"max_tokens": 16000}),
        )
        .await;
    assert_eq!(status, 402, "{text}");
    assert!(text.contains("run budget exhausted"));
    assert!(
        h.seen.lock().unwrap().is_empty(),
        "nothing reached the upstream"
    );
    assert_eq!(h.status().await["run"]["denied"], 1);
}

#[tokio::test]
async fn refuses_calls_beyond_the_total_cap_with_402() {
    let h = Harness::start("total", 5.0, 0.01).await;
    let (status, text) = h
        .chat(
            "/v1/chat/completions",
            "hello",
            json!({"max_tokens": 16000}),
        )
        .await;
    assert_eq!(status, 402);
    assert!(text.contains("total budget exhausted"));
}

#[tokio::test]
async fn a_breach_halts_later_calls() {
    let h = Harness::start("breach", 5.0, 50.0).await;
    let (status, _) = h.chat("/v1/chat/completions", "overcost", json!({})).await;
    assert_eq!(status, 200);
    let (status, _) = h.chat("/v1/chat/completions", "hello", json!({})).await;
    assert_eq!(status, 402);
}

#[tokio::test]
async fn upstream_rejections_cost_nothing_and_crashes_keep_the_reservation() {
    let h = Harness::start("errors", 5.0, 50.0).await;
    assert_eq!(
        h.chat("/v1/chat/completions", "reject", json!({})).await.0,
        400
    );
    let ledgers = h.status().await;
    assert!(
        ledgers["run"]["charged_usd"].is_null()
            || ledgers["run"]["uncertain_usd"].as_f64().unwrap() == 0.0
    );
    assert_eq!(
        h.chat("/v1/chat/completions", "crash", json!({})).await.0,
        500
    );
    assert!(h.status().await["run"]["uncertain_usd"].as_f64().unwrap() > 0.0);
    let rows = h.meter();
    assert!(rows[1]["error"].as_str().unwrap().contains("oops"));
}

#[tokio::test]
async fn prices_token_counts_when_cost_is_missing() {
    let h = Harness::start("tokens", 5.0, 50.0).await;
    assert_eq!(
        h.chat("/v1/chat/completions", "tokens", json!({})).await.0,
        200
    );
    let settled = h.status().await["run"]["settled_usd"].as_f64().unwrap();
    assert!((settled - (1000.0 * 0.30e-6 + 1000.0 * 1.0e-6)).abs() < 1e-12);
}

#[tokio::test]
async fn refuses_unknown_models_and_bad_json() {
    let h = Harness::start("refuse", 5.0, 50.0).await;
    let (status, _) = h
        .chat(
            "/v1/chat/completions",
            "hello",
            json!({"model": "openai/gpt-5.2"}),
        )
        .await;
    assert_eq!(status, 403);
    let response = h
        .client
        .post(format!("{}/v1/chat/completions", h.base))
        .body("{")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 400);
    assert_eq!(h.meter().len(), 2);
}

#[tokio::test]
async fn serves_embeddings_and_lists_models() {
    let h = Harness::start("embed", 5.0, 50.0).await;
    let response = h
        .client
        .post(format!("{}/__tag/ingest/cortex/v1/embeddings", h.base))
        .json(&json!({"model": "openai/text-embedding-3-small", "input": "x"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let response = h
        .client
        .post(format!("{}/v1/embeddings", h.base))
        .json(&json!({"model": "openai/text-embedding-3-small", "input": "x"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let models: Value = h
        .client
        .get(format!("{}/v1/models", h.base))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(models["data"].as_array().unwrap().len(), 2);
    assert_eq!(h.meter()[0]["phase"], "ingest");
}

#[tokio::test]
async fn a_second_gateway_cannot_share_the_total_ledger() {
    let h = Harness::start("locked", 5.0, 50.0).await;
    let options = Options {
        run_dir: h.dir.join("run-2"),
        ledger_total: h.dir.join("ledger-total.json"),
        run_limit: 5.0,
        total_limit: 50.0,
        listen: "127.0.0.1:0".parse().unwrap(),
        upstream: "http://127.0.0.1:9/v1".into(),
        capture: false,
    };
    let error = Bound::open(&options, "k".into()).await.unwrap_err();
    assert!(matches!(error, ohrsi_gateway::Error::Locked(_)), "{error}");
}

#[tokio::test]
async fn unreachable_upstreams_return_502_and_keep_the_reservation() {
    let dir = std::env::temp_dir().join(format!("ohrsi-gw-flow-{}-down", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let options = Options {
        run_dir: dir.join("run"),
        ledger_total: dir.join("total.json"),
        run_limit: 5.0,
        total_limit: 50.0,
        listen: "127.0.0.1:0".parse().unwrap(),
        upstream: "http://127.0.0.1:1/v1".into(),
        capture: false,
    };
    let bound = Bound::open(&options, "k".into()).await.unwrap();
    let base = format!("http://{}", bound.addr);
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        bound
            .serve(async {
                let _ = rx.await;
            })
            .await
            .unwrap();
    });
    let response = reqwest::Client::new()
        .post(format!("{base}/v1/chat/completions"))
        .json(&json!({"model": "z-ai/glm-5.3-flash", "messages": []}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 502);
    let ledgers: Value = reqwest::get(format!("{base}/status"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(ledgers["run"]["uncertain_usd"].as_f64().unwrap() > 0.0);
    let _ = tx.send(());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_client_that_disconnects_mid_call_still_settles() {
    let h = Harness::start("disconnect", 5.0, 50.0).await;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(50))
        .build()
        .unwrap();
    let body =
        json!({"model": "z-ai/glm-5.3-flash", "messages": [{"role": "user", "content": "slow"}]});
    let result = client
        .post(format!("{}/v1/chat/completions", h.base))
        .json(&body)
        .send()
        .await;
    assert!(
        result.is_err(),
        "the client gives up before the upstream answers"
    );
    for _ in 0..100 {
        let run = h.status().await["run"].clone();
        if run["in_flight_usd"].as_f64().unwrap() < 1e-12 {
            assert!(
                (run["settled_usd"].as_f64().unwrap() - 0.003).abs() < 1e-12,
                "{run}"
            );
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("reservation never settled: {}", h.status().await);
}
