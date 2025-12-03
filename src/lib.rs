//! # Payments Engine
//!
//! A streaming transaction processor that handles deposits, withdrawals,
//! disputes, resolves, and chargebacks for client accounts.
//!
//! ## Design Principles
//!
//! - **Fixed-point arithmetic**: Uses 4 decimal places via `rust_decimal`
//! - **Streaming processing**: Memory-efficient CSV processing
//! - **Strict invariants**: `total == available + held` always maintained
//! - **Deterministic output**: Accounts sorted by client ID
//!
//! ## Example
//!
//! ```no_run
//! use payments_engine::PaymentsEngine;
//! use std::io::Cursor;
//!
//! let csv = "type,client,tx,amount\ndeposit,1,1,100.0\n";
//! let mut engine = PaymentsEngine::new();
//! engine.process_csv(Cursor::new(csv)).unwrap();
//! engine.write_output(std::io::stdout()).unwrap();
//! ```

pub mod account;
pub mod decimal;
pub mod engine;
pub mod error;
pub mod transaction;

pub use account::ClientAccount;
pub use decimal::Decimal4;
pub use engine::PaymentsEngine;
pub use error::{EngineError, Result};
pub use transaction::{ParsedTransaction, StoredTransaction, TransactionRecord, TxKind};
