//! This module contains various trait impls needed when benchmarking.

use super::*;
use rand::distributions::{Distribution, Standard};
use rand::Rng;

impl Distribution<TransactionType> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> TransactionType {
        match rng.gen_range(0..=4) {
            0 => TransactionType::Deposit { amount: rng.gen() },
            1 => TransactionType::Withdrawal { amount: rng.gen() },
            2 => TransactionType::Dispute,
            3 => TransactionType::Resolve,
            _ => TransactionType::Chargeback,
        }
    }
}

impl Distribution<Transaction> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Transaction {
        Transaction {
            tx_type: rng.gen(),
            client: rng.gen(),
            tx: rng.gen(),
        }
    }
}
