<h1 align="center">Corken</h1>
Corken is a payments engine able to replay transactions from a csv file, outputting all the final account states from the engine.

## Features
[x] Withdrawals
[x] Deposits
[ ] Disputes
    [x] Deposit disputes
    [ ] Withdrawal disputes
[x] Dispute resolution
[x] Chargebacks

## Running (from source)
Corken has no external dependencies and will compile on rustc nightly, as well as stable.
  1. `git clone https://github.com/vgarleanu/corken`
  2. `cd corken && cargo run --release -- transactions.csv`

## Testing
Corken comes bundled with a couple of unit tests to ensure the logic behind the engine is sound. To run the unit tests, simply execute:
  1. `cargo test`

## Benchmarking
To benchmark this program run:
  1. `cargo bench`
