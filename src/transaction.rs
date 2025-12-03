//! Transaction models for CSV parsing and internal representation.

use crate::decimal::Decimal4;
use serde::Deserialize;
use std::str::FromStr;

/// Raw transaction record as read from CSV.
///
/// Uses string-based parsing for flexibility and handles the optional amount field
/// which is only present for deposit and withdrawal transactions.
#[derive(Debug, Deserialize)]
pub struct TransactionRecord {
    /// Transaction type: deposit, withdrawal, dispute, resolve, chargeback
    #[serde(rename = "type")]
    pub tx_type: String,

    /// Client ID (u16)
    pub client: u16,

    /// Transaction ID (globally unique, u32)
    pub tx: u32,

    /// Amount (present for deposit/withdrawal, absent for dispute/resolve/chargeback)
    pub amount: Option<String>,
}

impl TransactionRecord {
    /// Parses the raw CSV record into a typed transaction.
    ///
    /// Returns `None` if the record is invalid (unknown type, missing amount, etc.).
    pub fn parse(&self) -> Option<ParsedTransaction> {
        let tx_type = self.tx_type.trim().to_lowercase();

        match tx_type.as_str() {
            "deposit" => {
                let amount = self.parse_amount()?;
                Some(ParsedTransaction {
                    tx_id: self.tx,
                    client: self.client,
                    kind: TxKind::Deposit(amount),
                })
            }
            "withdrawal" => {
                let amount = self.parse_amount()?;
                Some(ParsedTransaction {
                    tx_id: self.tx,
                    client: self.client,
                    kind: TxKind::Withdrawal(amount),
                })
            }
            "dispute" => Some(ParsedTransaction {
                tx_id: self.tx,
                client: self.client,
                kind: TxKind::Dispute,
            }),
            "resolve" => Some(ParsedTransaction {
                tx_id: self.tx,
                client: self.client,
                kind: TxKind::Resolve,
            }),
            "chargeback" => Some(ParsedTransaction {
                tx_id: self.tx,
                client: self.client,
                kind: TxKind::Chargeback,
            }),
            _ => None,
        }
    }

    /// Parses the amount field into a `Decimal4`.
    fn parse_amount(&self) -> Option<Decimal4> {
        let amount_str = self.amount.as_ref()?;
        let trimmed = amount_str.trim();
        if trimmed.is_empty() {
            return None;
        }
        Decimal4::from_str(trimmed).ok()
    }
}

/// A parsed and validated transaction ready for processing.
#[derive(Debug, Clone)]
pub struct ParsedTransaction {
    /// Globally unique transaction ID
    pub tx_id: u32,

    /// Client ID
    pub client: u16,

    /// Transaction type with associated data
    pub kind: TxKind,
}

/// Transaction type variants with associated data.
#[derive(Debug, Clone)]
pub enum TxKind {
    /// Credit funds to client account.
    Deposit(Decimal4),

    /// Debit funds from client account (if sufficient available balance).
    Withdrawal(Decimal4),

    /// Claim that referenced transaction was erroneous; holds disputed funds.
    Dispute,

    /// Resolution in client's favor; releases held funds back to available.
    Resolve,

    /// Resolution against client; removes held funds and locks account.
    Chargeback,
}

/// A stored transaction for dispute reference.
///
/// Only deposit transactions are stored, as disputes reference prior deposits
/// to determine the amount to hold/release/chargeback.
#[derive(Debug, Clone)]
pub struct StoredTransaction {
    /// Transaction ID
    pub tx_id: u32,

    /// Client who owns this transaction
    pub client: u16,

    /// Original transaction amount
    pub amount: Decimal4,

    /// Whether this transaction is currently under dispute
    pub under_dispute: bool,
}

impl StoredTransaction {
    /// Creates a new stored transaction from a deposit.
    pub fn from_deposit(tx_id: u32, client: u16, amount: Decimal4) -> Self {
        StoredTransaction {
            tx_id,
            client,
            amount,
            under_dispute: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_deposit() {
        let record = TransactionRecord {
            tx_type: "deposit".to_string(),
            client: 1,
            tx: 100,
            amount: Some("10.5".to_string()),
        };

        let parsed = record.parse().unwrap();
        assert_eq!(parsed.tx_id, 100);
        assert_eq!(parsed.client, 1);
        match parsed.kind {
            TxKind::Deposit(amt) => assert_eq!(amt.to_string(), "10.5000"),
            _ => panic!("Expected Deposit"),
        }
    }

    #[test]
    fn test_parse_withdrawal() {
        let record = TransactionRecord {
            tx_type: "withdrawal".to_string(),
            client: 2,
            tx: 200,
            amount: Some("5.25".to_string()),
        };

        let parsed = record.parse().unwrap();
        match parsed.kind {
            TxKind::Withdrawal(amt) => assert_eq!(amt.to_string(), "5.2500"),
            _ => panic!("Expected Withdrawal"),
        }
    }

    #[test]
    fn test_parse_dispute() {
        let record = TransactionRecord {
            tx_type: "dispute".to_string(),
            client: 1,
            tx: 100,
            amount: None,
        };

        let parsed = record.parse().unwrap();
        assert!(matches!(parsed.kind, TxKind::Dispute));
    }

    #[test]
    fn test_parse_handles_whitespace() {
        let record = TransactionRecord {
            tx_type: "  deposit  ".to_string(),
            client: 1,
            tx: 100,
            amount: Some("  10.0  ".to_string()),
        };

        let parsed = record.parse().unwrap();
        match parsed.kind {
            TxKind::Deposit(amt) => assert_eq!(amt.to_string(), "10.0000"),
            _ => panic!("Expected Deposit"),
        }
    }

    #[test]
    fn test_parse_rejects_unknown_type() {
        let record = TransactionRecord {
            tx_type: "unknown".to_string(),
            client: 1,
            tx: 100,
            amount: Some("10.0".to_string()),
        };

        assert!(record.parse().is_none());
    }

    #[test]
    fn test_parse_rejects_missing_amount_for_deposit() {
        let record = TransactionRecord {
            tx_type: "deposit".to_string(),
            client: 1,
            tx: 100,
            amount: None,
        };

        assert!(record.parse().is_none());
    }
}
