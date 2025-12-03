//! Payments Engine CLI
//!
//! A streaming transaction processor that reads CSV input and outputs
//! final client account states.
//!
//! # Usage
//!
//! ```bash
//! cargo run -- transactions.csv > accounts.csv
//! ```
//!
//! # Environment Variables
//!
//! - `RUST_LOG`: Set to `debug` or `warn` to control logging verbosity

use payments_engine::{EngineError, PaymentsEngine, Result};
use std::env;
use std::fs::File;
use std::io::{self, BufReader};
use std::process;

fn main() {
    env_logger::init();

    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        return Err(EngineError::MissingArgument);
    }

    let input_path = &args[1];
    let file = File::open(input_path)?;
    let reader = BufReader::new(file);

    let mut engine = PaymentsEngine::new();
    engine.process_csv(reader)?;

    let stdout = io::stdout();
    let handle = stdout.lock();
    engine.write_output(handle)?;

    Ok(())
}
