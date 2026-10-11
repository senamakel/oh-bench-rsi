//! Command-line options and the serve loop for the `ohrsi-gateway` binary.
//!
//! ```text
//! ohrsi-gateway --run-dir runs/<run> [--ledger-total runs/ledger-total.json]
//!               [--run-limit 5] [--total-limit 50] [--listen 127.0.0.1:8790]
//!               [--upstream https://openrouter.ai/api/v1]
//! ```
//!
//! The provider key comes from `OPENROUTER_API_KEY` and never appears in
//! arguments or logs. The gateway prints `listening on http://<addr>` once
//! bound, then serves until interrupted.

use std::net::SocketAddr;
use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::ledger::{Budgets, Ledger};
use crate::lock::LedgerLock;
use crate::meter::Meter;
use crate::price::PriceTable;
use crate::proxy::{Gateway, GatewayConfig};
use crate::request::Limits;

/// Parsed command-line options.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// Directory holding this run's `budget.json` and `meter.jsonl`.
    pub run_dir: PathBuf,
    /// The campaign-wide ledger.
    pub ledger_total: PathBuf,
    /// Per-run dollar cap.
    pub run_limit: f64,
    /// Campaign-wide dollar cap.
    pub total_limit: f64,
    /// Listen address; must be loopback.
    pub listen: SocketAddr,
    /// Upstream base URL.
    pub upstream: String,
}

/// Parses arguments (without the program name).
///
/// # Errors
///
/// Returns [`Error::Usage`] for unknown flags, missing values, an
/// unparsable number or address, a non-loopback address, or a missing
/// `--run-dir`.
pub fn parse(args: &[String]) -> Result<Options> {
    let mut run_dir = None;
    let mut options = Options {
        run_dir: PathBuf::new(),
        ledger_total: PathBuf::from("runs/ledger-total.json"),
        run_limit: 5.0,
        total_limit: 50.0,
        listen: SocketAddr::from(([127, 0, 0, 1], 8790)),
        upstream: "https://openrouter.ai/api/v1".to_owned(),
    };
    let mut iter = args.iter();
    while let Some(flag) = iter.next() {
        let mut value = || {
            iter.next()
                .cloned()
                .ok_or_else(|| Error::Usage(format!("{flag} needs a value")))
        };
        let number = |text: String| {
            text.parse::<f64>()
                .map_err(|_| Error::Usage(format!("{flag}: {text:?} is not a number")))
        };
        match flag.as_str() {
            "--run-dir" => run_dir = Some(PathBuf::from(value()?)),
            "--ledger-total" => options.ledger_total = PathBuf::from(value()?),
            "--run-limit" => options.run_limit = number(value()?)?,
            "--total-limit" => options.total_limit = number(value()?)?,
            "--listen" => {
                let text = value()?;
                options.listen = text
                    .parse()
                    .map_err(|_| Error::Usage(format!("--listen: {text:?} is not an address")))?;
            }
            "--upstream" => options.upstream = value()?,
            other => return Err(Error::Usage(format!("unknown argument {other:?}"))),
        }
    }
    if !options.listen.ip().is_loopback() {
        return Err(Error::Usage("--listen must be a loopback address".into()));
    }
    options.run_dir = run_dir.ok_or_else(|| Error::Usage("--run-dir is required".into()))?;
    Ok(options)
}

/// A bound gateway, ready to serve.
#[derive(Debug)]
pub struct Bound {
    /// The address actually bound.
    pub addr: SocketAddr,
    listener: tokio::net::TcpListener,
    gateway: std::sync::Arc<Gateway>,
    _lock: LedgerLock,
}

impl Bound {
    /// Locks the total ledger, opens both ledgers and the meter, and binds.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Locked`] if another gateway owns the total ledger,
    /// any ledger error, or [`Error::Io`] if the address cannot be bound.
    pub async fn open(options: &Options, api_key: String) -> Result<Self> {
        let lock = LedgerLock::acquire(&options.ledger_total)?;
        let budgets = Budgets::new(
            Ledger::open(&options.run_dir.join("budget.json"), options.run_limit)?,
            Ledger::open(&options.ledger_total, options.total_limit)?,
        );
        let meter = Meter::open(&options.run_dir.join("meter.jsonl"))?;
        let config = GatewayConfig {
            upstream: options.upstream.clone(),
            api_key,
            prices: PriceTable::standard(),
            limits: Limits::default(),
        };
        let listener = tokio::net::TcpListener::bind(options.listen)
            .await
            .map_err(|source| Error::Io {
                path: PathBuf::from(options.listen.to_string()),
                source,
            })?;
        let addr = listener.local_addr().map_err(|source| Error::Io {
            path: PathBuf::from(options.listen.to_string()),
            source,
        })?;
        Ok(Self {
            addr,
            listener,
            gateway: Gateway::new(config, budgets, meter),
            _lock: lock,
        })
    }

    /// Serves until `shutdown` resolves, then releases the lock.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the server fails.
    pub async fn serve(self, shutdown: impl Future<Output = ()> + Send + 'static) -> Result<()> {
        let addr = self.addr;
        axum::serve(self.listener, self.gateway.router())
            .with_graceful_shutdown(shutdown)
            .await
            .map_err(|source| Error::Io {
                path: PathBuf::from(addr.to_string()),
                source,
            })
    }
}

#[cfg(test)]
mod test;
