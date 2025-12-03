//! Comprehensive edge case tests for the payments engine.
//!
//! This file tests all possible edge cases to ensure robust handling.

use std::io::Cursor;

// Re-implement the test helper since we can't easily import from the lib tests
fn run_csv(csv: &str) -> String {
    use payments_engine::PaymentsEngine;

    let mut engine = PaymentsEngine::new();
    engine.process_csv(Cursor::new(csv)).unwrap();

    let mut output = Vec::new();
    engine.write_output(&mut output).unwrap();
    String::from_utf8(output).unwrap()
}

fn get_account_line(output: &str, client_id: u16) -> Option<String> {
    output
        .lines()
        .skip(1) // Skip header
        .find(|line| line.starts_with(&format!("{},", client_id)))
        .map(|s| s.to_string())
}

fn parse_account(line: &str) -> (String, String, String, bool) {
    let parts: Vec<&str> = line.split(',').collect();
    (
        parts[1].to_string(), // available
        parts[2].to_string(), // held
        parts[3].to_string(), // total
        parts[4] == "true",   // locked
    )
}

// ==================== DEPOSIT EDGE CASES ====================

#[test]
fn test_deposit_zero_amount() {
    let csv = r#"type,client,tx,amount
deposit,1,1,0.0
deposit,1,2,10.0"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, total, _) = parse_account(&line);

    // Zero deposit should be allowed but has no effect
    assert_eq!(available, "10.0000");
    assert_eq!(total, "10.0000");
}

#[test]
fn test_deposit_very_small_amount() {
    let csv = r#"type,client,tx,amount
deposit,1,1,0.0001"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, total, _) = parse_account(&line);

    assert_eq!(available, "0.0001");
    assert_eq!(total, "0.0001");
}

#[test]
fn test_deposit_max_precision() {
    let csv = r#"type,client,tx,amount
deposit,1,1,1.1234"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, _, _) = parse_account(&line);

    assert_eq!(available, "1.1234");
}

#[test]
fn test_deposit_large_amount() {
    let csv = r#"type,client,tx,amount
deposit,1,1,999999999999.9999"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, _, _) = parse_account(&line);

    assert_eq!(available, "999999999999.9999");
}

#[test]
fn test_multiple_deposits_same_client() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
deposit,1,2,200.0
deposit,1,3,300.0"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, total, _) = parse_account(&line);

    assert_eq!(available, "600.0000");
    assert_eq!(total, "600.0000");
}

// ==================== WITHDRAWAL EDGE CASES ====================

#[test]
fn test_withdrawal_exact_balance() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
withdrawal,1,2,100.0"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, total, _) = parse_account(&line);

    assert_eq!(available, "0.0000");
    assert_eq!(total, "0.0000");
}

#[test]
fn test_withdrawal_zero_amount() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
withdrawal,1,2,0.0"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, _, _) = parse_account(&line);

    // Zero withdrawal should have no effect
    assert_eq!(available, "100.0000");
}

#[test]
fn test_withdrawal_exceeds_balance_by_tiny_amount() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
withdrawal,1,2,100.0001"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, _, _) = parse_account(&line);

    // Withdrawal should fail - balance unchanged
    assert_eq!(available, "100.0000");
}

#[test]
fn test_withdrawal_from_empty_account() {
    let csv = r#"type,client,tx,amount
deposit,1,1,0.0
withdrawal,1,2,1.0"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, _, _) = parse_account(&line);

    assert_eq!(available, "0.0000");
}

#[test]
fn test_withdrawal_without_prior_deposit() {
    let csv = r#"type,client,tx,amount
withdrawal,1,1,100.0"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, total, _) = parse_account(&line);

    // Account created but withdrawal fails
    assert_eq!(available, "0.0000");
    assert_eq!(total, "0.0000");
}

#[test]
fn test_multiple_withdrawals() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
withdrawal,1,2,30.0
withdrawal,1,3,20.0
withdrawal,1,4,10.0"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, _, _) = parse_account(&line);

    assert_eq!(available, "40.0000");
}

// ==================== DISPUTE EDGE CASES ====================

#[test]
fn test_dispute_nonexistent_transaction() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
dispute,1,999,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, held, _, _) = parse_account(&line);

    // Dispute ignored - no funds held
    assert_eq!(available, "100.0000");
    assert_eq!(held, "0.0000");
}

#[test]
fn test_dispute_wrong_client() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
deposit,2,2,50.0
dispute,2,1,"#;

    let output = run_csv(csv);
    let line1 = get_account_line(&output, 1).unwrap();
    let (available1, held1, _, _) = parse_account(&line1);

    // Dispute ignored - client 2 can't dispute client 1's transaction
    assert_eq!(available1, "100.0000");
    assert_eq!(held1, "0.0000");
}

#[test]
fn test_double_dispute_same_transaction() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
dispute,1,1,
dispute,1,1,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, held, _, _) = parse_account(&line);

    // Second dispute should be ignored
    assert_eq!(available, "0.0000");
    assert_eq!(held, "100.0000");
}

#[test]
fn test_dispute_after_partial_withdrawal() {
    // Client deposits 100, withdraws 70, then deposit is disputed
    // Available becomes -70, held becomes 100
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
withdrawal,1,2,70.0
dispute,1,1,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, held, total, _) = parse_account(&line);

    // Available goes negative (they owe money)
    assert_eq!(available, "-70.0000");
    assert_eq!(held, "100.0000");
    assert_eq!(total, "30.0000"); // Still 30 total
}

#[test]
fn test_dispute_after_full_withdrawal() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
withdrawal,1,2,100.0
dispute,1,1,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, held, total, _) = parse_account(&line);

    // Available goes negative
    assert_eq!(available, "-100.0000");
    assert_eq!(held, "100.0000");
    assert_eq!(total, "0.0000");
}

#[test]
fn test_dispute_multiple_deposits() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
deposit,1,2,50.0
dispute,1,1,
dispute,1,2,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, held, total, _) = parse_account(&line);

    assert_eq!(available, "0.0000");
    assert_eq!(held, "150.0000");
    assert_eq!(total, "150.0000");
}

// ==================== RESOLVE EDGE CASES ====================

#[test]
fn test_resolve_nonexistent_transaction() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
dispute,1,1,
resolve,1,999,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, held, _, _) = parse_account(&line);

    // Resolve ignored - funds still held
    assert_eq!(available, "0.0000");
    assert_eq!(held, "100.0000");
}

#[test]
fn test_resolve_not_disputed() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
resolve,1,1,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, held, _, _) = parse_account(&line);

    // Resolve ignored - transaction not under dispute
    assert_eq!(available, "100.0000");
    assert_eq!(held, "0.0000");
}

#[test]
fn test_resolve_wrong_client() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
deposit,2,2,50.0
dispute,1,1,
resolve,2,1,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, held, _, _) = parse_account(&line);

    // Resolve ignored - wrong client
    assert_eq!(available, "0.0000");
    assert_eq!(held, "100.0000");
}

#[test]
fn test_double_resolve() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
dispute,1,1,
resolve,1,1,
resolve,1,1,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, held, _, _) = parse_account(&line);

    // Second resolve should be ignored
    assert_eq!(available, "100.0000");
    assert_eq!(held, "0.0000");
}

#[test]
fn test_dispute_after_resolve() {
    // Can a transaction be re-disputed after being resolved?
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
dispute,1,1,
resolve,1,1,
dispute,1,1,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, held, _, locked) = parse_account(&line);

    // Transaction can be re-disputed
    assert_eq!(available, "0.0000");
    assert_eq!(held, "100.0000");
    assert!(!locked);
}

// ==================== CHARGEBACK EDGE CASES ====================

#[test]
fn test_chargeback_nonexistent_transaction() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
dispute,1,1,
chargeback,1,999,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (_, _, _, locked) = parse_account(&line);

    // Chargeback ignored - account not locked
    assert!(!locked);
}

#[test]
fn test_chargeback_not_disputed() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
chargeback,1,1,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, _, locked) = parse_account(&line);

    // Chargeback ignored - transaction not under dispute
    assert_eq!(available, "100.0000");
    assert!(!locked);
}

#[test]
fn test_chargeback_wrong_client() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
deposit,2,2,50.0
dispute,1,1,
chargeback,2,1,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (_, held, _, locked) = parse_account(&line);

    // Chargeback ignored - wrong client
    assert_eq!(held, "100.0000");
    assert!(!locked);
}

#[test]
fn test_chargeback_after_resolve() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
dispute,1,1,
resolve,1,1,
chargeback,1,1,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, _, locked) = parse_account(&line);

    // Chargeback ignored - no longer under dispute
    assert_eq!(available, "100.0000");
    assert!(!locked);
}

#[test]
fn test_double_chargeback() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
dispute,1,1,
chargeback,1,1,
chargeback,1,1,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, held, total, locked) = parse_account(&line);

    // Second chargeback should be ignored (account already locked)
    assert_eq!(available, "0.0000");
    assert_eq!(held, "0.0000");
    assert_eq!(total, "0.0000");
    assert!(locked);
}

#[test]
fn test_chargeback_with_remaining_balance() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
deposit,1,2,50.0
dispute,1,1,
chargeback,1,1,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, held, total, locked) = parse_account(&line);

    // Only disputed amount removed, account locked
    assert_eq!(available, "50.0000");
    assert_eq!(held, "0.0000");
    assert_eq!(total, "50.0000");
    assert!(locked);
}

// ==================== LOCKED ACCOUNT EDGE CASES ====================

#[test]
fn test_locked_account_ignores_deposit() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
dispute,1,1,
chargeback,1,1,
deposit,1,2,500.0"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (_, _, total, locked) = parse_account(&line);

    assert!(locked);
    assert_eq!(total, "0.0000"); // No new deposit
}

#[test]
fn test_locked_account_ignores_withdrawal() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
deposit,1,2,50.0
dispute,1,1,
chargeback,1,1,
withdrawal,1,3,25.0"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, _, locked) = parse_account(&line);

    assert!(locked);
    assert_eq!(available, "50.0000"); // No withdrawal
}

#[test]
fn test_locked_account_ignores_dispute() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
deposit,1,2,50.0
dispute,1,1,
chargeback,1,1,
dispute,1,2,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, held, _, locked) = parse_account(&line);

    assert!(locked);
    assert_eq!(available, "50.0000");
    assert_eq!(held, "0.0000"); // No new dispute
}

#[test]
fn test_locked_account_ignores_resolve() {
    // This is an edge case: what if there's an active dispute when account is locked?
    // Let's test by having two disputes, chargebacking one
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
deposit,1,2,50.0
dispute,1,1,
dispute,1,2,
chargeback,1,1,
resolve,1,2,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, held, total, locked) = parse_account(&line);

    // Account is locked, resolve should be ignored
    assert!(locked);
    // tx1: 100 chargebacked, tx2: 50 still held
    assert_eq!(available, "0.0000");
    assert_eq!(held, "50.0000");
    assert_eq!(total, "50.0000");
}

// ==================== CLIENT ID EDGE CASES ====================

#[test]
fn test_client_id_zero() {
    let csv = r#"type,client,tx,amount
deposit,0,1,100.0"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 0).unwrap();
    let (available, _, _, _) = parse_account(&line);

    assert_eq!(available, "100.0000");
}

#[test]
fn test_client_id_max_u16() {
    let csv = r#"type,client,tx,amount
deposit,65535,1,100.0"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 65535).unwrap();
    let (available, _, _, _) = parse_account(&line);

    assert_eq!(available, "100.0000");
}

#[test]
fn test_many_clients() {
    let mut csv = String::from("type,client,tx,amount\n");
    for i in 0..100 {
        csv.push_str(&format!("deposit,{},{},10.0\n", i, i));
    }

    let output = run_csv(&csv);
    let line_count = output.lines().count();

    // Header + 100 clients
    assert_eq!(line_count, 101);
}

// ==================== TRANSACTION ID EDGE CASES ====================

#[test]
fn test_tx_id_zero() {
    let csv = r#"type,client,tx,amount
deposit,1,0,100.0"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, _, _) = parse_account(&line);

    assert_eq!(available, "100.0000");
}

#[test]
fn test_tx_id_max_u32() {
    let csv = r#"type,client,tx,amount
deposit,1,4294967295,100.0"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, _, _) = parse_account(&line);

    assert_eq!(available, "100.0000");
}

#[test]
fn test_duplicate_tx_id_deposit() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
deposit,1,1,50.0"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, _, _) = parse_account(&line);

    // Second deposit with same tx id should be ignored
    assert_eq!(available, "100.0000");
}

#[test]
fn test_duplicate_tx_id_different_clients() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
deposit,2,1,50.0"#;

    let output = run_csv(csv);
    let line1 = get_account_line(&output, 1).unwrap();
    let (available1, _, _, _) = parse_account(&line1);

    // Second deposit with same tx id should be ignored even for different client
    assert_eq!(available1, "100.0000");

    // Client 2 should have an account but with 0 balance (or not exist)
    let line2 = get_account_line(&output, 2);
    if let Some(line) = line2 {
        let (available2, _, _, _) = parse_account(&line);
        assert_eq!(available2, "0.0000");
    }
}

// ==================== CSV FORMAT EDGE CASES ====================

#[test]
fn test_empty_csv_with_header() {
    let csv = "type,client,tx,amount\n";

    let output = run_csv(csv);
    let line_count = output.lines().count();

    // Just header, no accounts
    assert_eq!(line_count, 1);
    assert!(output.contains("client,available,held,total,locked"));
}

#[test]
fn test_csv_with_extra_whitespace() {
    let csv = "type,  client,   tx,    amount\n  deposit  ,  1  ,  1  ,   100.0  \n";

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, _, _) = parse_account(&line);

    assert_eq!(available, "100.0000");
}

#[test]
fn test_csv_with_mixed_case_type() {
    let csv = r#"type,client,tx,amount
DEPOSIT,1,1,100.0
Withdrawal,1,2,30.0
DISPUTE,1,1,
Resolve,1,1,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, held, _, _) = parse_account(&line);

    assert_eq!(available, "70.0000");
    assert_eq!(held, "0.0000");
}

#[test]
fn test_csv_with_empty_amount_for_deposit() {
    // Should be skipped
    let csv = r#"type,client,tx,amount
deposit,1,1,
deposit,1,2,100.0"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, _, _) = parse_account(&line);

    assert_eq!(available, "100.0000");
}

#[test]
fn test_csv_with_invalid_amount() {
    let csv = r#"type,client,tx,amount
deposit,1,1,abc
deposit,1,2,100.0"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, _, _) = parse_account(&line);

    assert_eq!(available, "100.0000");
}

// ==================== DECIMAL PRECISION EDGE CASES ====================

#[test]
fn test_decimal_precision_preserved() {
    let csv = r#"type,client,tx,amount
deposit,1,1,0.1234
deposit,1,2,0.5678"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, _, _) = parse_account(&line);

    assert_eq!(available, "0.6912");
}

#[test]
fn test_decimal_various_formats() {
    let csv = r#"type,client,tx,amount
deposit,1,1,1
deposit,1,2,2.5
deposit,1,3,3.50
deposit,1,4,4.1234"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, _, _, _) = parse_account(&line);

    // 1 + 2.5 + 3.5 + 4.1234 = 11.1234
    assert_eq!(available, "11.1234");
}

// ==================== COMPLEX SCENARIOS ====================

#[test]
fn test_multiple_disputes_on_different_transactions() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
deposit,1,2,50.0
deposit,1,3,25.0
dispute,1,1,
dispute,1,3,
resolve,1,1,
chargeback,1,3,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, held, total, locked) = parse_account(&line);

    // tx1: 100 disputed then resolved -> available
    // tx2: 50 never disputed -> available
    // tx3: 25 disputed then chargebacked -> removed
    assert_eq!(available, "150.0000");
    assert_eq!(held, "0.0000");
    assert_eq!(total, "150.0000");
    assert!(locked);
}

#[test]
fn test_interleaved_operations_multiple_clients() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
deposit,2,2,200.0
withdrawal,1,3,50.0
dispute,2,2,
deposit,1,4,25.0
chargeback,2,2,
withdrawal,1,5,30.0"#;

    let output = run_csv(csv);

    let line1 = get_account_line(&output, 1).unwrap();
    let (available1, _, total1, locked1) = parse_account(&line1);
    assert_eq!(available1, "45.0000"); // 100 - 50 + 25 - 30
    assert_eq!(total1, "45.0000");
    assert!(!locked1);

    let line2 = get_account_line(&output, 2).unwrap();
    let (available2, _, total2, locked2) = parse_account(&line2);
    assert_eq!(available2, "0.0000");
    assert_eq!(total2, "0.0000");
    assert!(locked2);
}

#[test]
fn test_dispute_resolve_dispute_chargeback_cycle() {
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
dispute,1,1,
resolve,1,1,
dispute,1,1,
chargeback,1,1,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, held, total, locked) = parse_account(&line);

    assert_eq!(available, "0.0000");
    assert_eq!(held, "0.0000");
    assert_eq!(total, "0.0000");
    assert!(locked);
}

#[test]
fn test_withdrawal_not_disputable() {
    // Withdrawals are not stored for dispute reference
    let csv = r#"type,client,tx,amount
deposit,1,1,100.0
withdrawal,1,2,30.0
dispute,1,2,"#;

    let output = run_csv(csv);
    let line = get_account_line(&output, 1).unwrap();
    let (available, held, _, _) = parse_account(&line);

    // Dispute on withdrawal tx should be ignored
    assert_eq!(available, "70.0000");
    assert_eq!(held, "0.0000");
}

// ==================== OUTPUT FORMAT VERIFICATION ====================

#[test]
fn test_output_sorted_by_client_id() {
    let csv = r#"type,client,tx,amount
deposit,5,1,50.0
deposit,1,2,10.0
deposit,3,3,30.0
deposit,2,4,20.0
deposit,4,5,40.0"#;

    let output = run_csv(csv);
    let lines: Vec<&str> = output.lines().collect();

    // Should be: header, 1, 2, 3, 4, 5
    assert!(lines[1].starts_with("1,"));
    assert!(lines[2].starts_with("2,"));
    assert!(lines[3].starts_with("3,"));
    assert!(lines[4].starts_with("4,"));
    assert!(lines[5].starts_with("5,"));
}

#[test]
fn test_output_always_four_decimal_places() {
    let csv = r#"type,client,tx,amount
deposit,1,1,1
deposit,2,2,2.5
deposit,3,3,3.12
deposit,4,4,4.123
deposit,5,5,5.1234"#;

    let output = run_csv(csv);

    // All amounts should have exactly 4 decimal places
    for line in output.lines().skip(1) {
        let parts: Vec<&str> = line.split(',').collect();
        // Check available, held, total (indices 1, 2, 3)
        for i in 1..=3 {
            let decimal_part = parts[i].split('.').nth(1).unwrap();
            assert_eq!(
                decimal_part.len(),
                4,
                "Field {} should have 4 decimal places: {}",
                i,
                parts[i]
            );
        }
    }
}
