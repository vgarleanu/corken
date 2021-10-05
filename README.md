<h1 align="center">Corken</h1>
Corken is a payments engine able to replay transactions from a csv file, outputting all the final account states from the engine.

## Features
1. [x] Withdrawals
2. [x] Deposits
3. [ ] Disputes
    1. [x] Deposit disputes
    2. [ ] Withdrawal disputes
4. [x] Dispute resolution
5. [x] Chargebacks
6. [ ] Accurate fp operations.

## Running (from source)
Corken has no external dependencies and will compile on rustc nightly (2021-09-07).
  1. `git clone https://github.com/vgarleanu/corken`
  2. `cd corken && cargo run --release -- transactions.csv`

## Testing
Corken comes bundled with a couple of unit tests to ensure the logic behind the engine is sound. To run the unit tests, simply execute:
  1. `cargo test`

## Benchmarking
To benchmark this program run:
  1. `cargo bench`
