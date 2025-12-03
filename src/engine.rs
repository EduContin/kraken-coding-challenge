//! Core payments processing engine.
//!
//! Processes transactions in chronological order and maintains client account states.
//! The engine uses streaming CSV processing and stores only deposit transactions
//! for dispute reference.

use crate::account::ClientAccount;
use crate::decimal::Decimal4;
use crate::error::Result;
use crate::transaction::{ParsedTransaction, StoredTransaction, TransactionRecord, TxKind};
use csv::{ReaderBuilder, Trim};
use log::{debug, warn};
use std::collections::HashMap;
use std::io::{Read, Write};

/// The payments processing engine.
///
/// Maintains client accounts and stored transactions for dispute resolution.
/// Processes transactions in the order they are received (assumed chronological).
///
/// # Output Ordering
///
/// Final account states are output sorted by client ID in ascending order
/// to ensure deterministic, reproducible output.
pub struct PaymentsEngine {
    /// Client accounts indexed by client ID.
    accounts: HashMap<u16, ClientAccount>,

    /// Stored deposit transactions for dispute/resolve/chargeback reference.
    transactions: HashMap<u32, StoredTransaction>,
}

impl PaymentsEngine {
    /// Creates a new empty engine.
    pub fn new() -> Self {
        PaymentsEngine {
            accounts: HashMap::new(),
            transactions: HashMap::new(),
        }
    }

    /// Processes transactions from a CSV reader in streaming fashion.
    ///
    /// Records are read one at a time to minimize memory usage.
    /// Invalid records are logged at warn level and skipped.
    pub fn process_csv<R: Read>(&mut self, reader: R) -> Result<()> {
        let mut csv_reader = ReaderBuilder::new()
            .trim(Trim::All)
            .flexible(true)
            .from_reader(reader);

        for (row_idx, result) in csv_reader.deserialize::<TransactionRecord>().enumerate() {
            let row_num = row_idx + 2; // 1-indexed, accounting for header row

            match result {
                Ok(record) => {
                    if let Some(tx) = record.parse() {
                        if let Err(e) = self.process_transaction(tx, row_num) {
                            warn!("Row {}: {}", row_num, e);
                        }
                    } else {
                        warn!("Row {}: Failed to parse transaction record", row_num);
                    }
                }
                Err(e) => {
                    warn!("Row {}: CSV parse error: {}", row_num, e);
                }
            }
        }

        Ok(())
    }

    /// Processes a single parsed transaction.
    fn process_transaction(&mut self, tx: ParsedTransaction, row: usize) -> Result<()> {
        match tx.kind {
            TxKind::Deposit(amount) => {
                self.ensure_account_exists(tx.client);

                // Safety: account was just created/verified above
                if self
                    .accounts
                    .get(&tx.client)
                    .expect("account exists")
                    .is_locked()
                {
                    debug!(
                        "Row {}: Ignoring deposit for locked account {}",
                        row, tx.client
                    );
                    return Ok(());
                }
                self.process_deposit(tx.tx_id, tx.client, amount, row)?;
            }
            TxKind::Withdrawal(amount) => {
                self.ensure_account_exists(tx.client);

                // Safety: account was just created/verified above
                if self
                    .accounts
                    .get(&tx.client)
                    .expect("account exists")
                    .is_locked()
                {
                    debug!(
                        "Row {}: Ignoring withdrawal for locked account {}",
                        row, tx.client
                    );
                    return Ok(());
                }
                self.process_withdrawal(tx.tx_id, tx.client, amount, row)?;
            }
            TxKind::Dispute => {
                if self.is_account_locked(tx.client) {
                    debug!(
                        "Row {}: Ignoring dispute for locked account {}",
                        row, tx.client
                    );
                    return Ok(());
                }
                self.process_dispute(tx.tx_id, tx.client, row)?;
            }
            TxKind::Resolve => {
                if self.is_account_locked(tx.client) {
                    debug!(
                        "Row {}: Ignoring resolve for locked account {}",
                        row, tx.client
                    );
                    return Ok(());
                }
                self.process_resolve(tx.tx_id, tx.client, row)?;
            }
            TxKind::Chargeback => {
                if self.is_account_locked(tx.client) {
                    debug!(
                        "Row {}: Ignoring chargeback for locked account {}",
                        row, tx.client
                    );
                    return Ok(());
                }
                self.process_chargeback(tx.tx_id, tx.client, row)?;
            }
        }

        Ok(())
    }

    /// Ensures an account exists for the given client, creating one if needed.
    fn ensure_account_exists(&mut self, client: u16) {
        self.accounts
            .entry(client)
            .or_insert_with(|| ClientAccount::new(client));
    }

    /// Checks if an account exists and is locked.
    fn is_account_locked(&self, client: u16) -> bool {
        self.accounts
            .get(&client)
            .map(|a| a.is_locked())
            .unwrap_or(false)
    }

    /// Processes a deposit transaction.
    fn process_deposit(
        &mut self,
        tx_id: u32,
        client: u16,
        amount: Decimal4,
        row: usize,
    ) -> Result<()> {
        if self.transactions.contains_key(&tx_id) {
            warn!("Row {}: Duplicate transaction ID {}, ignoring", row, tx_id);
            return Ok(());
        }

        // Safety: ensure_account_exists was called before this method
        let account = self.accounts.get_mut(&client).expect("account exists");

        if account.deposit(amount) {
            self.transactions.insert(
                tx_id,
                StoredTransaction::from_deposit(tx_id, client, amount),
            );
            debug!("Row {}: Deposited {} to client {}", row, amount, client);
        }

        Ok(())
    }

    /// Processes a withdrawal transaction.
    fn process_withdrawal(
        &mut self,
        tx_id: u32,
        client: u16,
        amount: Decimal4,
        row: usize,
    ) -> Result<()> {
        if self.transactions.contains_key(&tx_id) {
            warn!("Row {}: Duplicate transaction ID {}, ignoring", row, tx_id);
            return Ok(());
        }

        // Safety: ensure_account_exists was called before this method
        let account = self.accounts.get_mut(&client).expect("account exists");

        if account.withdraw(amount) {
            debug!("Row {}: Withdrew {} from client {}", row, amount, client);
        } else {
            debug!(
                "Row {}: Withdrawal of {} from client {} failed (insufficient funds)",
                row, amount, client
            );
        }

        Ok(())
    }

    /// Processes a dispute transaction.
    ///
    /// A dispute moves funds from available to held. If the client has withdrawn
    /// funds after the disputed deposit, available may become negative.
    fn process_dispute(&mut self, tx_id: u32, client: u16, row: usize) -> Result<()> {
        let stored_tx = match self.transactions.get_mut(&tx_id) {
            Some(tx) => tx,
            None => {
                debug!(
                    "Row {}: Dispute references unknown transaction {}, ignoring",
                    row, tx_id
                );
                return Ok(());
            }
        };

        if stored_tx.client != client {
            warn!(
                "Row {}: Dispute client {} doesn't match transaction client {}, ignoring",
                row, client, stored_tx.client
            );
            return Ok(());
        }

        if stored_tx.under_dispute {
            debug!(
                "Row {}: Transaction {} already under dispute, ignoring",
                row, tx_id
            );
            return Ok(());
        }

        let amount = stored_tx.amount;
        stored_tx.under_dispute = true;

        // Safety: disputes reference stored transactions which require an existing account
        let account = self
            .accounts
            .get_mut(&client)
            .expect("account exists for stored tx");
        account.hold(amount);

        debug!(
            "Row {}: Disputed transaction {} for client {}, holding {}",
            row, tx_id, client, amount
        );

        Ok(())
    }

    /// Processes a resolve transaction.
    fn process_resolve(&mut self, tx_id: u32, client: u16, row: usize) -> Result<()> {
        let stored_tx = match self.transactions.get_mut(&tx_id) {
            Some(tx) => tx,
            None => {
                debug!(
                    "Row {}: Resolve references unknown transaction {}, ignoring",
                    row, tx_id
                );
                return Ok(());
            }
        };

        if stored_tx.client != client {
            warn!(
                "Row {}: Resolve client {} doesn't match transaction client {}, ignoring",
                row, client, stored_tx.client
            );
            return Ok(());
        }

        if !stored_tx.under_dispute {
            debug!(
                "Row {}: Transaction {} not under dispute, ignoring resolve",
                row, tx_id
            );
            return Ok(());
        }

        let amount = stored_tx.amount;
        stored_tx.under_dispute = false;

        // Safety: resolves reference stored transactions which require an existing account
        let account = self
            .accounts
            .get_mut(&client)
            .expect("account exists for stored tx");
        account.release(amount);

        debug!(
            "Row {}: Resolved dispute for transaction {} for client {}, released {}",
            row, tx_id, client, amount
        );

        Ok(())
    }

    /// Processes a chargeback transaction.
    fn process_chargeback(&mut self, tx_id: u32, client: u16, row: usize) -> Result<()> {
        let stored_tx = match self.transactions.get_mut(&tx_id) {
            Some(tx) => tx,
            None => {
                debug!(
                    "Row {}: Chargeback references unknown transaction {}, ignoring",
                    row, tx_id
                );
                return Ok(());
            }
        };

        if stored_tx.client != client {
            warn!(
                "Row {}: Chargeback client {} doesn't match transaction client {}, ignoring",
                row, client, stored_tx.client
            );
            return Ok(());
        }

        if !stored_tx.under_dispute {
            debug!(
                "Row {}: Transaction {} not under dispute, ignoring chargeback",
                row, tx_id
            );
            return Ok(());
        }

        let amount = stored_tx.amount;
        stored_tx.under_dispute = false;

        // Safety: chargebacks reference stored transactions which require an existing account
        let account = self
            .accounts
            .get_mut(&client)
            .expect("account exists for stored tx");
        account.chargeback(amount);

        debug!(
            "Row {}: Chargeback for transaction {} for client {}, removed {}, account locked",
            row, tx_id, client, amount
        );

        Ok(())
    }

    /// Writes final account states to CSV.
    ///
    /// Output is sorted by client ID in ascending order for deterministic results.
    /// All monetary values are formatted with exactly 4 decimal places.
    pub fn write_output<W: Write>(&self, writer: W) -> Result<()> {
        let mut csv_writer = csv::Writer::from_writer(writer);

        csv_writer.write_record(["client", "available", "held", "total", "locked"])?;

        // Sort by client ID for deterministic output
        let mut accounts: Vec<_> = self.accounts.values().collect();
        accounts.sort_by_key(|a| a.client);

        for account in accounts {
            csv_writer.write_record([
                account.client.to_string(),
                account.available.to_string(),
                account.held.to_string(),
                account.total.to_string(),
                account.locked.to_string(),
            ])?;
        }

        csv_writer.flush()?;
        Ok(())
    }

    /// Returns a reference to an account (for testing).
    #[cfg(test)]
    pub fn get_account(&self, client_id: u16) -> Option<&ClientAccount> {
        self.accounts.get(&client_id)
    }
}

impl Default for PaymentsEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn process_csv_str(csv: &str) -> PaymentsEngine {
        let mut engine = PaymentsEngine::new();
        engine.process_csv(Cursor::new(csv)).unwrap();
        engine
    }

    #[test]
    fn test_simple_deposits() {
        let csv = r#"type,client,tx,amount
deposit,1,1,10.0
deposit,1,2,5.0
deposit,2,3,20.0"#;

        let engine = process_csv_str(csv);

        let acc1 = engine.get_account(1).unwrap();
        assert_eq!(acc1.available.to_string(), "15.0000");
        assert_eq!(acc1.total.to_string(), "15.0000");

        let acc2 = engine.get_account(2).unwrap();
        assert_eq!(acc2.available.to_string(), "20.0000");
    }

    #[test]
    fn test_withdrawal() {
        let csv = r#"type,client,tx,amount
deposit,1,1,10.0
withdrawal,1,2,3.0"#;

        let engine = process_csv_str(csv);
        let acc = engine.get_account(1).unwrap();
        assert_eq!(acc.available.to_string(), "7.0000");
        assert_eq!(acc.total.to_string(), "7.0000");
    }

    #[test]
    fn test_withdrawal_insufficient_funds() {
        let csv = r#"type,client,tx,amount
deposit,1,1,10.0
withdrawal,1,2,15.0"#;

        let engine = process_csv_str(csv);
        let acc = engine.get_account(1).unwrap();
        assert_eq!(acc.available.to_string(), "10.0000");
    }

    #[test]
    fn test_dispute_resolve() {
        let csv = r#"type,client,tx,amount
deposit,1,1,10.0
dispute,1,1,
resolve,1,1,"#;

        let engine = process_csv_str(csv);
        let acc = engine.get_account(1).unwrap();
        assert_eq!(acc.available.to_string(), "10.0000");
        assert_eq!(acc.held.to_string(), "0.0000");
        assert!(!acc.locked);
    }

    #[test]
    fn test_dispute_chargeback() {
        let csv = r#"type,client,tx,amount
deposit,1,1,10.0
dispute,1,1,
chargeback,1,1,"#;

        let engine = process_csv_str(csv);
        let acc = engine.get_account(1).unwrap();
        assert_eq!(acc.available.to_string(), "0.0000");
        assert_eq!(acc.held.to_string(), "0.0000");
        assert_eq!(acc.total.to_string(), "0.0000");
        assert!(acc.locked);
    }

    #[test]
    fn test_locked_account_ignores_transactions() {
        let csv = r#"type,client,tx,amount
deposit,1,1,10.0
dispute,1,1,
chargeback,1,1,
deposit,1,2,100.0"#;

        let engine = process_csv_str(csv);
        let acc = engine.get_account(1).unwrap();
        assert_eq!(acc.total.to_string(), "0.0000");
        assert!(acc.locked);
    }

    #[test]
    fn test_dispute_wrong_client() {
        let csv = r#"type,client,tx,amount
deposit,1,1,10.0
dispute,2,1,"#;

        let engine = process_csv_str(csv);
        let acc = engine.get_account(1).unwrap();
        assert_eq!(acc.available.to_string(), "10.0000");
        assert_eq!(acc.held.to_string(), "0.0000");
    }

    #[test]
    fn test_whitespace_handling() {
        let csv = r#"type, client, tx, amount
deposit, 1, 1, 10.0
withdrawal, 1, 2, 3.0"#;

        let engine = process_csv_str(csv);
        let acc = engine.get_account(1).unwrap();
        assert_eq!(acc.available.to_string(), "7.0000");
    }

    #[test]
    fn test_sample_from_spec() {
        let csv = r#"type,client,tx,amount
deposit,1,1,1.0
deposit,2,2,2.0
deposit,1,3,2.0
withdrawal,1,4,1.5
withdrawal,2,5,3.0"#;

        let engine = process_csv_str(csv);

        let acc1 = engine.get_account(1).unwrap();
        assert_eq!(acc1.available.to_string(), "1.5000");
        assert_eq!(acc1.total.to_string(), "1.5000");

        let acc2 = engine.get_account(2).unwrap();
        assert_eq!(acc2.available.to_string(), "2.0000");
        assert_eq!(acc2.total.to_string(), "2.0000");
    }

    #[test]
    fn test_output_format() {
        let csv = r#"type,client,tx,amount
deposit,1,1,1.0
deposit,2,2,2.0"#;

        let engine = process_csv_str(csv);
        let mut output = Vec::new();
        engine.write_output(&mut output).unwrap();

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("client,available,held,total,locked"));
        assert!(output_str.contains("1,1.0000,0.0000,1.0000,false"));
        assert!(output_str.contains("2,2.0000,0.0000,2.0000,false"));
    }
}
