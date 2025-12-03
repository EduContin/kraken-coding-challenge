//! Error types for the payments engine.

use thiserror::Error;

/// Result type alias for engine operations
pub type Result<T> = std::result::Result<T, EngineError>;

/// Errors that can occur during engine operation.
#[derive(Error, Debug)]
pub enum EngineError {
    /// Failed to open or read the input file
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// CSV parsing error
    #[error("CSV parsing error: {0}")]
    Csv(#[from] csv::Error),

    /// Invalid transaction record
    #[error("Invalid transaction at row {row}: {message}")]
    InvalidRecord { row: usize, message: String },

    /// Duplicate transaction ID
    #[error("Duplicate transaction ID {tx_id} at row {row}")]
    DuplicateTxId { tx_id: u32, row: usize },

    /// Missing input file argument
    #[error("Missing input file argument. Usage: payments-engine <input.csv>")]
    MissingArgument,
}
