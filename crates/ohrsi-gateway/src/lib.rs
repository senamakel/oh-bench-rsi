//! # ohrsi-gateway
//!
//! The single spend choke point for an RSI run. Every model call the run
//! makes — swarm agents, the benchmark reader and judge, CortexDB enrichment
//! and embeddings — goes through this OpenAI-compatible loopback proxy to
//! OpenRouter, which:
//!
//! 1. allows only the models in its [`PriceTable`];
//! 2. pins each model to one provider with a price ceiling;
//! 3. reserves the call's worst-case cost against **both** the per-run and
//!    the campaign-wide [`Ledger`], refusing with HTTP 402 if it does not fit;
//! 4. settles to the cost OpenRouter reports, keeping the reservation when
//!    the cost is unknown;
//! 5. writes a [`MeterRow`] tagged with the caller's phase, seat and
//!    candidate.
//!
//! ```
//! use ohrsi_gateway::{Budgets, Ledger};
//! # let dir = std::env::temp_dir().join(format!("ohrsi-gw-doc-{}", std::process::id()));
//! let mut budgets = Budgets::new(
//!     Ledger::open(&dir.join("run/budget.json"), 5.0)?,
//!     Ledger::open(&dir.join("ledger-total.json"), 50.0)?,
//! );
//! let held = budgets.reserve(0.02)?;
//! budgets.settle(held, Some(0.004))?;
//! assert!(budgets.states().1.charged_usd() < 0.005);
//! # std::fs::remove_dir_all(&dir).ok();
//! # Ok::<(), ohrsi_gateway::Error>(())
//! ```
//!
//! ## What this crate does not do
//!
//! It does not decide *what* to spend on — that is the loop driver's job —
//! and it never exposes the provider key to clients, which receive a dummy
//! bearer. It refuses image content because the current run configuration
//! is text-only.

pub mod error;
pub mod ledger;
pub mod lock;
pub mod meter;
pub mod price;
pub mod proxy;
pub mod request;
pub mod server;
pub mod usage;

pub use error::{Error, Result};
pub use ledger::{Budgets, Ledger, LedgerState, Reservation};
pub use lock::LedgerLock;
pub use meter::{Meter, MeterRow, Tags};
pub use price::{EMBEDDING, GLM_FLASH, Price, PriceTable};
pub use proxy::{Gateway, GatewayConfig};
pub use request::{Endpoint, Limits, Prepared, prepare};
pub use server::{Bound, Options};
pub use usage::{SseUsage, Usage};
