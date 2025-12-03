//! Integration tests for the payments engine CLI.
//!
//! These tests run the actual binary and verify output against expected CSV files.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

/// Get path to test data file
fn test_data_path(filename: &str) -> String {
    format!("tests/data/{}", filename)
}

/// Run the binary with the given input file and return stdout
fn run_engine(input_file: &str) -> String {
    let mut cmd = Command::cargo_bin("payments-engine").unwrap();
    let assert = cmd.arg(input_file).assert().success();
    String::from_utf8(assert.get_output().stdout.clone()).unwrap()
}

/// Normalize CSV for comparison (sort lines, trim whitespace)
fn normalize_csv(csv: &str) -> Vec<String> {
    let mut lines: Vec<String> = csv
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    // Keep header first, sort the rest
    if lines.len() > 1 {
        let header = lines.remove(0);
        lines.sort();
        lines.insert(0, header);
    }

    lines
}

#[test]
fn test_sample_a_simple_deposits_withdrawals() {
    let output = run_engine(&test_data_path("sample_a.csv"));
    let expected = fs::read_to_string(test_data_path("expected_a.csv")).unwrap();

    let output_lines = normalize_csv(&output);
    let expected_lines = normalize_csv(&expected);

    assert_eq!(output_lines, expected_lines);
}

#[test]
fn test_sample_b_dispute_resolve_chargeback() {
    let output = run_engine(&test_data_path("sample_b_dispute.csv"));
    let expected = fs::read_to_string(test_data_path("expected_b.csv")).unwrap();

    let output_lines = normalize_csv(&output);
    let expected_lines = normalize_csv(&expected);

    assert_eq!(output_lines, expected_lines);
}

#[test]
fn test_sample_c_whitespace_handling() {
    let output = run_engine(&test_data_path("sample_c_whitespace.csv"));
    let expected = fs::read_to_string(test_data_path("expected_c.csv")).unwrap();

    let output_lines = normalize_csv(&output);
    let expected_lines = normalize_csv(&expected);

    assert_eq!(output_lines, expected_lines);
}

#[test]
fn test_sample_d_edge_cases() {
    let output = run_engine(&test_data_path("sample_d_edge_cases.csv"));
    let expected = fs::read_to_string(test_data_path("expected_d.csv")).unwrap();

    let output_lines = normalize_csv(&output);
    let expected_lines = normalize_csv(&expected);

    assert_eq!(output_lines, expected_lines);
}

#[test]
fn test_missing_file_error() {
    let mut cmd = Command::cargo_bin("payments-engine").unwrap();
    cmd.arg("nonexistent.csv")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error").or(predicate::str::contains("Error")));
}

#[test]
fn test_missing_argument_error() {
    let mut cmd = Command::cargo_bin("payments-engine").unwrap();
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Missing input file"));
}

#[test]
fn test_output_has_correct_header() {
    let output = run_engine(&test_data_path("sample_a.csv"));
    assert!(output.starts_with("client,available,held,total,locked"));
}

#[test]
fn test_decimal_precision_four_places() {
    let output = run_engine(&test_data_path("sample_a.csv"));

    // Check that values have 4 decimal places
    for line in output.lines().skip(1) {
        // Skip header
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 4 {
            // available, held, total should have exactly 4 decimal places
            for part in &parts[1..4] {
                if let Some(dot_pos) = part.find('.') {
                    let decimal_places = part.len() - dot_pos - 1;
                    assert_eq!(decimal_places, 4, "Expected 4 decimal places in: {}", part);
                }
            }
        }
    }
}
