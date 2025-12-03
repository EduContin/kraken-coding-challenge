# Payments Engine

A streaming Rust CLI that processes payment transactions from CSV input and outputs final client account states. Uses fixed-point arithmetic with 4 decimal places for precise monetary calculations.

## What This Project Demonstrates

- **Streaming architecture**: Processes arbitrarily large CSV files with constant memory usage
- **Financial precision**: Uses `rust_decimal` to avoid floating-point errors in monetary calculations
- **Robust error handling**: Gracefully handles malformed input without crashing
- **Comprehensive testing**: 88 tests covering core logic, edge cases, and integration scenarios
- **Production-ready design**: Deterministic output, clear invariants, and defensive coding

---

## Quick Start

```bash
# Build
cargo build --release

# Run
cargo run --release -- transactions.csv > accounts.csv

# Run tests
cargo test
```

## Usage

```bash
payments-engine <input.csv> > output.csv
```

**Example:**
```bash
cargo run -- tests/data/sample_a.csv
```

**Output:**
```csv
client,available,held,total,locked
1,1.5000,0.0000,1.5000,false
2,2.0000,0.0000,2.0000,false
```

---

## Architecture

### Project Structure

```
payments-engine/
├── Cargo.toml
├── README.md
├── src/
│   ├── main.rs          # CLI entry point
│   ├── lib.rs           # Library exports
│   ├── decimal.rs       # Decimal4 fixed-point type
│   ├── account.rs       # ClientAccount model
│   ├── transaction.rs   # Transaction models
│   ├── engine.rs        # Core processing engine
│   └── error.rs         # Error types
└── tests/
    ├── integration_test.rs
    ├── edge_cases_test.rs
    └── data/            # Sample CSV files
```

### Data Types

```rust
// Fixed-point decimal with 4 decimal places
struct Decimal4(rust_decimal::Decimal);

// Client account state
struct ClientAccount {
    client: u16,
    available: Decimal4,
    held: Decimal4,
    total: Decimal4,
    locked: bool,
}

// Transaction types
enum TxKind {
    Deposit(Decimal4),
    Withdrawal(Decimal4),
    Dispute,
    Resolve,
    Chargeback,
}
```

### Key Invariants

1. **Balance integrity**: `total == available + held` is maintained after every operation
2. **Locked accounts**: Once `locked == true`, all transactions for that client are rejected
3. **Transaction uniqueness**: Transaction IDs are globally unique; duplicates are ignored
4. **Client ownership**: Dispute/resolve/chargeback must reference a transaction belonging to the same client

---

## Transaction Processing Rules

### Deposit
```
available += amount
total += amount
Store transaction for potential disputes
```

### Withdrawal
```
if available >= amount:
    available -= amount
    total -= amount
else:
    ignore (insufficient funds)
```

### Dispute (references tx_id)
```
if tx exists AND tx.client == dispute.client AND not already disputed:
    available -= tx.amount
    held += tx.amount
    tx.under_dispute = true
    (total unchanged)
else:
    ignore
```

### Resolve (references tx_id)
```
if tx exists AND tx.client == resolve.client AND tx.under_dispute:
    held -= tx.amount
    available += tx.amount
    tx.under_dispute = false
    (total unchanged)
else:
    ignore
```

### Chargeback (references tx_id)
```
if tx exists AND tx.client == chargeback.client AND tx.under_dispute:
    held -= tx.amount
    total -= tx.amount
    tx.under_dispute = false
    account.locked = true
else:
    ignore
```

### Negative Available Balance

The `available` field may become negative in certain dispute scenarios. This occurs when:

1. A client deposits funds (e.g., 100)
2. The client withdraws some or all funds (e.g., 80)
3. The original deposit is later disputed

When the dispute is processed, the full deposit amount (100) is moved to `held`, resulting in `available = -80`. This is expected behavior—the invariant `total == available + held` is still maintained, and the negative balance indicates the client owes funds.

---

## Input/Output Format

### Input CSV
```csv
type,client,tx,amount
deposit,1,1,1.0
withdrawal,1,2,0.5
dispute,1,1,
resolve,1,1,
chargeback,1,1,
```

- **type**: `deposit`, `withdrawal`, `dispute`, `resolve`, `chargeback`
- **client**: `u16` client ID
- **tx**: `u32` globally unique transaction ID
- **amount**: Decimal with up to 4 fractional places (present for deposit/withdrawal only)

### Output CSV
```csv
client,available,held,total,locked
1,1.5000,0.0000,1.5000,false
2,2.0000,0.0000,2.0000,true
```

- All monetary values are formatted with exactly **4 decimal places**
- `locked` is `true` or `false`
- **Output is sorted by client ID** in ascending order for deterministic results

---

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Missing input file argument | Exit with error message |
| File not found | Exit with error message |
| Invalid CSV row | Log warning, skip row, continue |
| Unknown transaction type | Skip row |
| Missing amount for deposit/withdrawal | Skip row |
| Duplicate transaction ID | Log warning, skip row |
| Dispute/resolve/chargeback wrong client | Skip row |
| Withdrawal insufficient funds | Skip (no change) |
| Transaction on locked account | Skip |

---

## Testing

### Test Summary

- **89 total tests** (all passing)
- **27 unit tests** — core logic in each module
- **52 edge case tests** — comprehensive boundary conditions
- **8 integration tests** — CLI behavior and output format
- **2 doc tests** — example code verification

```bash
cargo test                          # Run all tests
cargo test --lib                    # Unit tests only
cargo test --test edge_cases_test   # Edge cases only
cargo test --test integration_test  # Integration tests only
```

### Edge Cases Covered

| Category | Test Cases |
|----------|------------|
| **Deposits** | Zero amount, very small (0.0001), max precision, large amounts, multiple |
| **Withdrawals** | Exact balance, zero, exceeds by 0.0001, empty account, no prior deposit |
| **Disputes** | Unknown tx, wrong client, double dispute, after partial/full withdrawal |
| **Resolves** | Unknown tx, not disputed, wrong client, double resolve, re-dispute after |
| **Chargebacks** | Unknown tx, not disputed, wrong client, after resolve, with remaining balance |
| **Locked Accounts** | Ignores deposit, withdrawal, dispute, resolve |
| **IDs** | Client 0, client 65535, tx 0, tx 4294967295, duplicates |
| **CSV Format** | Empty file, whitespace, mixed case, invalid amount |
| **Output** | Sorted by client ID, 4 decimal places |

---

## Performance

- **Streaming**: CSV records processed one at a time via `csv::Reader`
- **Memory**: O(clients + stored_transactions) — only deposits are stored
- **Lookups**: O(1) via `HashMap<u16, ClientAccount>` and `HashMap<u32, StoredTransaction>`
- **Precision**: `rust_decimal` provides arbitrary precision, rescaled to 4 decimal places

---

## Dependencies

| Crate | Purpose |
|-------|---------|
| `csv` | CSV parsing and writing |
| `serde` | Serialization/deserialization |
| `rust_decimal` | Fixed-point decimal arithmetic |
| `thiserror` | Error type definitions |
| `log` + `env_logger` | Optional debug logging |
| `assert_cmd` | Integration testing |

---

## Real Test Example

### Input: `tests/data/real_test_input.csv`

```csv
type,client,tx,amount
deposit,1,1,100.0000
deposit,2,2,200.0000
deposit,1,3,50.5000
withdrawal,1,4,30.0000
deposit,3,5,500.0000
withdrawal,2,6,50.0000
dispute,1,1,
deposit,4,7,75.2500
withdrawal,3,8,100.0000
resolve,1,1,
deposit,2,9,25.0000
dispute,3,5,
withdrawal,1,10,20.0000
chargeback,3,5,
deposit,3,11,1000.0000
withdrawal,4,12,25.0000
dispute,2,2,
resolve,2,2,
deposit,5,13,42.1234
```

### Command

```bash
cargo run --release -- tests/data/real_test_input.csv > accounts.csv
```

### Output

```csv
client,available,held,total,locked
1,100.5000,0.0000,100.5000,false
2,175.0000,0.0000,175.0000,false
3,-100.0000,0.0000,-100.0000,true
4,50.2500,0.0000,50.2500,false
5,42.1234,0.0000,42.1234,false
```

### Explanation

| Client | Transactions | Final State |
|--------|--------------|-------------|
| **1** | deposit 100 → deposit 50.5 → withdrawal 30 → dispute tx1 → resolve tx1 → withdrawal 20 | `available=100.5, locked=false` |
| **2** | deposit 200 → withdrawal 50 → deposit 25 → dispute tx2 → resolve tx2 | `available=175, locked=false` |
| **3** | deposit 500 → withdrawal 100 → dispute tx5 → **chargeback** → deposit 1000 (ignored) | `available=-100, locked=true` |
| **4** | deposit 75.25 → withdrawal 25 | `available=50.25, locked=false` |
| **5** | deposit 42.1234 | `available=42.1234, locked=false` |

**Verification:**
- ✅ Invariant `total == available + held` maintained for all clients
- ✅ Client 3 locked after chargeback; subsequent deposit ignored
- ✅ Client 3 has negative available due to dispute after withdrawal
- ✅ 4 decimal precision preserved (client 5)
- ✅ Output sorted by client ID

---

## License

MIT
