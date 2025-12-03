//! Client account model and operations.
//!
//! Maintains the invariant: `total == available + held` at all times.

use crate::decimal::Decimal4;
use serde::Serialize;

/// Represents a client's account state.
///
/// # Invariants
///
/// - `total == available + held` is maintained after every operation
/// - Once `locked == true`, all further transactions are rejected
///
/// # Negative Available Balance
///
/// The `available` field may become negative in dispute scenarios. This occurs
/// when a client deposits funds, withdraws some or all of them, and then the
/// original deposit is disputed. The dispute moves the full deposit amount to
/// `held`, which can result in `available` going negative. The invariant
/// `total == available + held` is still maintained.
#[derive(Debug, Clone, Serialize)]
pub struct ClientAccount {
    /// Unique client identifier (u16).
    pub client: u16,

    /// Funds available for withdrawal. May be negative after disputes.
    pub available: Decimal4,

    /// Funds held due to active disputes.
    pub held: Decimal4,

    /// Total funds: `available + held`.
    pub total: Decimal4,

    /// Account frozen due to chargeback. No further transactions accepted.
    pub locked: bool,
}

impl ClientAccount {
    /// Creates a new account for a client with zero balances.
    pub fn new(client_id: u16) -> Self {
        ClientAccount {
            client: client_id,
            available: Decimal4::ZERO,
            held: Decimal4::ZERO,
            total: Decimal4::ZERO,
            locked: false,
        }
    }

    /// Returns `true` if the account is locked (frozen).
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Deposits funds into the account.
    ///
    /// Increases `available` and `total` by the given amount.
    /// Returns `false` if the account is locked.
    pub fn deposit(&mut self, amount: Decimal4) -> bool {
        if self.locked {
            return false;
        }

        self.available += amount;
        self.total += amount;
        true
    }

    /// Withdraws funds from the account.
    ///
    /// Returns `true` if the withdrawal succeeded, `false` if:
    /// - Account is locked
    /// - Insufficient available funds (`available < amount`)
    pub fn withdraw(&mut self, amount: Decimal4) -> bool {
        if self.locked {
            return false;
        }

        if self.available < amount {
            return false;
        }

        self.available -= amount;
        self.total -= amount;
        true
    }

    /// Holds funds for a dispute.
    ///
    /// Moves `amount` from `available` to `held`. The `total` remains unchanged.
    ///
    /// Note: `available` may become negative if the client has withdrawn funds
    /// after the disputed deposit. This is expected behavior.
    ///
    /// Returns `false` if the account is locked.
    pub fn hold(&mut self, amount: Decimal4) -> bool {
        if self.locked {
            return false;
        }

        self.available -= amount;
        self.held += amount;
        true
    }

    /// Releases held funds back to available (resolves a dispute).
    ///
    /// Moves `amount` from `held` back to `available`. The `total` remains unchanged.
    /// Returns `false` if the account is locked.
    pub fn release(&mut self, amount: Decimal4) -> bool {
        if self.locked {
            return false;
        }

        self.held -= amount;
        self.available += amount;
        true
    }

    /// Processes a chargeback.
    ///
    /// Removes `amount` from `held` and `total`, then locks the account.
    /// Returns `false` if the account is already locked.
    pub fn chargeback(&mut self, amount: Decimal4) -> bool {
        if self.locked {
            return false;
        }

        self.held -= amount;
        self.total -= amount;
        self.locked = true;
        true
    }

    /// Verifies the invariant: `total == available + held`.
    #[cfg(debug_assertions)]
    pub fn check_invariant(&self) -> bool {
        self.total == self.available + self.held
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn dec(s: &str) -> Decimal4 {
        Decimal4::from_str(s).unwrap()
    }

    #[test]
    fn test_new_account_has_zero_balances() {
        let account = ClientAccount::new(1);
        assert_eq!(account.client, 1);
        assert_eq!(account.available, Decimal4::ZERO);
        assert_eq!(account.held, Decimal4::ZERO);
        assert_eq!(account.total, Decimal4::ZERO);
        assert!(!account.locked);
    }

    #[test]
    fn test_deposit_increases_available_and_total() {
        let mut account = ClientAccount::new(1);
        assert!(account.deposit(dec("10.0")));

        assert_eq!(account.available.to_string(), "10.0000");
        assert_eq!(account.held.to_string(), "0.0000");
        assert_eq!(account.total.to_string(), "10.0000");
        assert!(account.check_invariant());
    }

    #[test]
    fn test_withdrawal_decreases_available_and_total() {
        let mut account = ClientAccount::new(1);
        account.deposit(dec("10.0"));
        assert!(account.withdraw(dec("3.5")));

        assert_eq!(account.available.to_string(), "6.5000");
        assert_eq!(account.total.to_string(), "6.5000");
        assert!(account.check_invariant());
    }

    #[test]
    fn test_withdrawal_fails_with_insufficient_funds() {
        let mut account = ClientAccount::new(1);
        account.deposit(dec("10.0"));
        assert!(!account.withdraw(dec("15.0")));

        assert_eq!(account.available.to_string(), "10.0000");
        assert_eq!(account.total.to_string(), "10.0000");
    }

    #[test]
    fn test_hold_and_release_cycle() {
        let mut account = ClientAccount::new(1);
        account.deposit(dec("10.0"));

        assert!(account.hold(dec("4.0")));
        assert_eq!(account.available.to_string(), "6.0000");
        assert_eq!(account.held.to_string(), "4.0000");
        assert_eq!(account.total.to_string(), "10.0000");
        assert!(account.check_invariant());

        assert!(account.release(dec("4.0")));
        assert_eq!(account.available.to_string(), "10.0000");
        assert_eq!(account.held.to_string(), "0.0000");
        assert_eq!(account.total.to_string(), "10.0000");
        assert!(account.check_invariant());
    }

    #[test]
    fn test_chargeback_removes_funds_and_locks() {
        let mut account = ClientAccount::new(1);
        account.deposit(dec("10.0"));
        account.hold(dec("4.0"));

        assert!(account.chargeback(dec("4.0")));
        assert_eq!(account.available.to_string(), "6.0000");
        assert_eq!(account.held.to_string(), "0.0000");
        assert_eq!(account.total.to_string(), "6.0000");
        assert!(account.locked);
        assert!(account.check_invariant());
    }

    #[test]
    fn test_locked_account_rejects_all_operations() {
        let mut account = ClientAccount::new(1);
        account.deposit(dec("10.0"));
        account.hold(dec("5.0"));
        account.chargeback(dec("5.0"));

        assert!(account.locked);

        assert!(!account.deposit(dec("1.0")));
        assert!(!account.withdraw(dec("1.0")));
        assert!(!account.hold(dec("1.0")));
        assert!(!account.release(dec("1.0")));
        assert!(!account.chargeback(dec("1.0")));

        assert_eq!(account.available.to_string(), "5.0000");
        assert_eq!(account.total.to_string(), "5.0000");
    }
}
