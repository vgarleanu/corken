#![doc = include_str!("../README.md")]
/// Errors returned by the state machine.
pub mod error;
/// Contains some trait impls necessary for generating random transactions for benchmarking.
pub mod rand_utils;

use serde::Deserialize;
use serde::Serialize;

use ahash::AHashMap;

use error::*;

/// Struct represents a transaction and contains its state.
#[derive(Clone, Debug, Deserialize)]
pub struct Transaction {
    /// Represents the transaction type.
    #[serde(flatten)]
    tx_type: TransactionType,
    /// Represents a client id.
    client: u16,
    /// Represents a transaction id.
    tx: u32,
}

/// Enum represents the state of a transaction dispute.
#[derive(Clone, Debug)]
pub enum DisputeState {
    /// The transaction is currently being disputed.
    Disputed,
    /// The transaction dispute has been resolved.
    Resolved,
}

/// Represents a transaction type. This would be deserialized from a `type` field in a serialized
/// file.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "type")]
pub enum TransactionType {
    /// Represents a deposit transaction, this enum field also embeds the amount thats been
    /// deposited.
    Deposit { amount: f64 },
    /// Represents a withdrawal transaction, this enum field also embeds the amount thats been
    /// withdrawn.
    Withdrawal { amount: f64 },
    /// Represents a dispute transaction.
    Dispute,
    /// Represents a resolve transaction.
    Resolve,
    /// Represents a chargeback transaction.
    Chargeback,
}

/// Struct represents an account in the state machine.
#[derive(Serialize, Default)]
pub struct Account {
    /// Field contains the ID of the client/account. This field gets renamed to `client` when
    /// serialized.
    #[serde(rename = "client")]
    id: u16,
    /// Represents the available balance of this account. This is the balance that the account can
    /// withdraw, or use.
    available: f64,
    /// Represents the held balance of this account. When a client files a dispute, this balance
    /// gets increased while the `available` balance gets decreased. The client cannot use this
    /// this balance.
    held: f64,
    /// Represents the total value/balance of this account.
    total: f64,
    /// Represents whether this account is locked or not.
    locked: bool,
}

/// Struct represents the state machine that can consume transactions. This state machine is
/// infinitely parallelizable.
pub struct State {
    /// Hashmap of all accounts indexed by the `id` field.
    accounts: AHashMap<u16, Account>,
    /// A cache of disputable transactions paired with a dispute state. If the dispute state is
    /// `None`, then the transaction is not under dispute.
    /// This cache is indexed by `Transaction::tx`.
    tx_cache: AHashMap<u32, (Transaction, Option<DisputeState>)>,
}

impl Default for State {
    fn default() -> State {
        State {
            accounts: AHashMap::with_capacity(1024),
            tx_cache: AHashMap::with_capacity(1024),
        }
    }
}

impl State {
    /// Function will construct the state machine and replay all the transactions from the iterator
    /// passed in.
    ///
    /// # Arguments
    /// * `txs` - Iterator over owned `Transactions`.
    ///
    /// # Notes
    /// If any transactions error out during execution, unless youre running in debug mode you will
    /// not see the errors logged. If you wish to handle the errors, or log them in-production,
    /// youre better off iterating over the iterator manually.
    pub fn from_iterator(txs: impl Iterator<Item = Transaction>) -> Self {
        let mut this = Self::default();
        for tx in txs {
            cfg_if::cfg_if! {
                if #[cfg(debug_assertions)] {
                    if let Err(e) = this.execute(tx.clone()) {
                        eprintln!("{:?} {:?}", tx, e);
                    }
                } else {
                    let _ = this.execute(tx.clone());
                }
            }
        }

        this
    }

    /// Function will execute a transaction, returning an error if the transaction failed to be
    /// commited.
    ///
    /// # Arguments
    /// * `tx` - Transaction to be executed.
    ///
    /// # Returns
    /// This function will return a `TxError` if various checks fail. If an error is returned, you
    /// can safely assume that no account data has been modified.
    pub fn execute(&mut self, tx: Transaction) -> Result<(), TxError> {
        // negative amounts are not allowed as they can flip balances.
        if matches!(tx.tx_type, TransactionType::Deposit { amount } | TransactionType::Withdrawal { amount } if amount < 0.0)
        {
            return Err(TxError::InternalError);
        }

        let mut account = self.accounts.entry(tx.client).or_insert(Account {
            id: tx.client,
            ..Account::default()
        });

        // NOTE (assumption): if an account gets locked, we probably want to ignore all future txs
        // from them until the account is manually unlocked.
        if account.locked {
            return Err(TxError::AccountLocked);
        }

        match tx.tx_type {
            TransactionType::Deposit { amount } => {
                account.available += amount;
                account.total += amount;
            }
            TransactionType::Withdrawal { amount } => {
                if account.available < amount {
                    return Err(TxError::NotEnoughFunds);
                }

                account.available -= amount;
                account.total -= amount;
            }
            TransactionType::Dispute => {
                let (disputed_tx, dispute_status) = self
                    .tx_cache
                    .get_mut(&tx.tx)
                    .ok_or(TxError::TxDoesntExist)?;

                if tx.client != disputed_tx.client {
                    return Err(TxError::Unauthorized);
                }

                if dispute_status.is_some() {
                    return Err(TxError::TxAlreadyDisputed);
                }

                let disputed_amount = match disputed_tx.tx_type {
                    TransactionType::Deposit { amount } => amount,
                    _ => return Err(TxError::InvalidDispute),
                };

                // NOTE: The spec doesnt specifically state what transactions can be disputed.
                // Based on the logic described in there for disputes, it is safe to assume that at
                // least Deposit transactions can be disputed.
                if matches!(disputed_tx.tx_type, TransactionType::Deposit { .. }) {
                    account.available -= disputed_amount;
                    account.held += disputed_amount;
                } else {
                    // The spec doesnt specify how we should handle the balance transfer for
                    // withdrawal disputes so we kinda just wing it here.
                    //
                    // For deposit transaction disputes the available balance is decreased while
                    // the held balance is increased, total balance is not affected. If we get a
                    // withdrawal dispute we can just increase the held balance and total balance.
                    account.held += disputed_amount;
                    account.total += disputed_amount;
                }

                *dispute_status = Some(DisputeState::Disputed);
            }
            TransactionType::Resolve => {
                let (disputed_tx, dispute_status) = self
                    .tx_cache
                    .get_mut(&tx.tx)
                    .ok_or(TxError::TxDoesntExist)?;

                if tx.client != disputed_tx.client {
                    return Err(TxError::Unauthorized);
                }

                let disputed_amount = match disputed_tx.tx_type {
                    TransactionType::Deposit { amount } => amount,
                    _ => return Err(TxError::InvalidDispute),
                };

                if !matches!(dispute_status, Some(DisputeState::Disputed)) {
                    return Err(TxError::TxNotUnderDispute);
                }

                account.held -= disputed_amount;
                account.available += disputed_amount;

                *dispute_status = Some(DisputeState::Resolved);
            }
            TransactionType::Chargeback => {
                let (disputed_tx, dispute_status) = self
                    .tx_cache
                    .get_mut(&tx.tx)
                    .ok_or(TxError::TxDoesntExist)?;

                if tx.client != disputed_tx.client {
                    return Err(TxError::Unauthorized);
                }

                let disputed_amount = match disputed_tx.tx_type {
                    TransactionType::Deposit { amount } => amount,
                    _ => return Err(TxError::InvalidDispute),
                };

                if !matches!(dispute_status, Some(DisputeState::Disputed)) {
                    return Err(TxError::TxNotUnderDispute);
                }

                account.held -= disputed_amount;
                account.total -= disputed_amount;
                account.locked = true;
                *dispute_status = Some(DisputeState::Resolved);
            }
        }

        // Transactions with disputes that have been resolved can now be safely removed from
        // `tx_cache` because they can never be disputed again.
        if matches!(
            tx.tx_type,
            TransactionType::Resolve | TransactionType::Chargeback
        ) {
            self.tx_cache.remove(&tx.tx);
        }

        if matches!(tx.tx_type, TransactionType::Deposit { .. }) {
            self.tx_cache.insert(tx.tx, (tx, None));
        }

        // NOTE: Sanity check
        debug_assert!((account.total - (account.held + account.available)).abs() < f64::EPSILON);

        Ok(())
    }

    pub fn accounts(&self) -> impl Iterator<Item = &Account> {
        self.accounts.values()
    }
}

#[cfg(test)]
pub mod test {
    use super::*;

    #[test]
    fn test_deposit() {
        let mut state = State::default();
        state
            .execute(Transaction {
                tx_type: TransactionType::Deposit { amount: 120.0 },
                client: 1,
                tx: 1,
            })
            .unwrap();

        let account = state.accounts.get(&1).unwrap();
        assert_eq!(account.total, 120.0);
        assert_eq!(account.available, 120.0);
    }

    #[test]
    fn test_withdrawals() {
        let mut state = State::default();
        state
            .execute(Transaction {
                tx_type: TransactionType::Deposit { amount: 120.0 },
                client: 1,
                tx: 1,
            })
            .unwrap();

        assert_eq!(
            state.execute(Transaction {
                tx_type: TransactionType::Withdrawal { amount: 240.0 },
                client: 1,
                tx: 2,
            }),
            Err(TxError::NotEnoughFunds)
        );

        state
            .execute(Transaction {
                tx_type: TransactionType::Withdrawal { amount: 120.0 },
                client: 1,
                tx: 2,
            })
            .unwrap();

        let account = state.accounts.get(&1).unwrap();
        assert_eq!(account.total, 0.0);
        assert_eq!(account.available, 0.0);
    }

    #[test]
    fn test_deposit_dispute() {
        let mut state = State::default();
        state
            .execute(Transaction {
                tx_type: TransactionType::Deposit { amount: 120.0 },
                client: 1,
                tx: 1,
            })
            .unwrap();

        state
            .execute(Transaction {
                tx_type: TransactionType::Dispute,
                client: 1,
                tx: 1,
            })
            .unwrap();

        {
            let account = state.accounts.get(&1).unwrap();
            assert_eq!(account.total, 120.0);
            assert_eq!(account.held, 120.0);
            assert_eq!(account.available, 0.0);
        }

        state
            .execute(Transaction {
                tx_type: TransactionType::Resolve,
                client: 1,
                tx: 1,
            })
            .unwrap();

        let account = state.accounts.get(&1).unwrap();
        assert_eq!(account.total, 120.0);
        assert_eq!(account.held, 0.0);
        assert_eq!(account.available, 120.0);
    }

    #[test]
    fn test_dispute_chargeback() {
        let mut state = State::default();
        state
            .execute(Transaction {
                tx_type: TransactionType::Deposit { amount: 120.0 },
                client: 1,
                tx: 1,
            })
            .unwrap();

        state
            .execute(Transaction {
                tx_type: TransactionType::Dispute,
                client: 1,
                tx: 1,
            })
            .unwrap();

        state
            .execute(Transaction {
                tx_type: TransactionType::Chargeback,
                client: 1,
                tx: 1,
            })
            .unwrap();

        let account = state.accounts.get(&1).unwrap();
        assert_eq!(account.total, 0.0);
        assert_eq!(account.held, 0.0);
        assert_eq!(account.available, 0.0);
        assert!(account.locked);
    }

    #[test]
    fn test_chargeback_after_resolve() {
        let mut state = State::default();
        state
            .execute(Transaction {
                tx_type: TransactionType::Deposit { amount: 120.0 },
                client: 1,
                tx: 1,
            })
            .unwrap();

        state
            .execute(Transaction {
                tx_type: TransactionType::Dispute,
                client: 1,
                tx: 1,
            })
            .unwrap();

        state
            .execute(Transaction {
                tx_type: TransactionType::Resolve,
                client: 1,
                tx: 1,
            })
            .unwrap();

        assert_eq!(
            state.execute(Transaction {
                tx_type: TransactionType::Chargeback,
                client: 1,
                tx: 1,
            }),
            Err(TxError::TxDoesntExist)
        );
    }

    #[test]
    fn test_double_chargeback() {
        let mut state = State::default();
        state
            .execute(Transaction {
                tx_type: TransactionType::Deposit { amount: 120.0 },
                client: 1,
                tx: 1,
            })
            .unwrap();

        state
            .execute(Transaction {
                tx_type: TransactionType::Dispute,
                client: 1,
                tx: 1,
            })
            .unwrap();

        state
            .execute(Transaction {
                tx_type: TransactionType::Chargeback,
                client: 1,
                tx: 1,
            })
            .unwrap();

        assert_eq!(
            state.execute(Transaction {
                tx_type: TransactionType::Chargeback,
                client: 1,
                tx: 1,
            }),
            Err(TxError::AccountLocked)
        );
    }

    #[test]
    fn test_double_resolve() {
        let mut state = State::default();
        state
            .execute(Transaction {
                tx_type: TransactionType::Deposit { amount: 120.0 },
                client: 1,
                tx: 1,
            })
            .unwrap();

        state
            .execute(Transaction {
                tx_type: TransactionType::Dispute,
                client: 1,
                tx: 1,
            })
            .unwrap();

        state
            .execute(Transaction {
                tx_type: TransactionType::Resolve,
                client: 1,
                tx: 1,
            })
            .unwrap();

        assert_eq!(
            state.execute(Transaction {
                tx_type: TransactionType::Resolve,
                client: 1,
                tx: 1,
            }),
            Err(TxError::TxDoesntExist)
        );
    }

    #[test]
    fn test_double_dispute() {
        let mut state = State::default();
        state
            .execute(Transaction {
                tx_type: TransactionType::Deposit { amount: 120.0 },
                client: 1,
                tx: 1,
            })
            .unwrap();

        state
            .execute(Transaction {
                tx_type: TransactionType::Dispute,
                client: 1,
                tx: 1,
            })
            .unwrap();

        state
            .execute(Transaction {
                tx_type: TransactionType::Resolve,
                client: 1,
                tx: 1,
            })
            .unwrap();

        assert_eq!(
            state.execute(Transaction {
                tx_type: TransactionType::Dispute,
                client: 1,
                tx: 1,
            }),
            Err(TxError::TxDoesntExist)
        );
    }

    #[test]
    fn test_dispute_from_different_uid() {
        let mut state = State::default();
        state
            .execute(Transaction {
                tx_type: TransactionType::Deposit { amount: 120.0 },
                client: 1,
                tx: 1,
            })
            .unwrap();

        assert_eq!(
            state.execute(Transaction {
                tx_type: TransactionType::Dispute,
                client: 2,
                tx: 1,
            }),
            Err(TxError::Unauthorized)
        );

        assert_eq!(
            state.execute(Transaction {
                tx_type: TransactionType::Resolve,
                client: 2,
                tx: 1,
            }),
            Err(TxError::Unauthorized)
        );

        assert_eq!(
            state.execute(Transaction {
                tx_type: TransactionType::Chargeback,
                client: 2,
                tx: 1,
            }),
            Err(TxError::Unauthorized)
        );
    }

    #[test]
    #[should_panic]
    fn test_withdrawal_dispute() {
        let mut state = State::default();
        state
            .execute(Transaction {
                tx_type: TransactionType::Deposit { amount: 120.0 },
                client: 1,
                tx: 1,
            })
            .unwrap();

        state
            .execute(Transaction {
                tx_type: TransactionType::Withdrawal { amount: 75.5 },
                client: 1,
                tx: 2,
            })
            .unwrap();

        state
            .execute(Transaction {
                tx_type: TransactionType::Dispute,
                client: 1,
                tx: 2,
            })
            .unwrap();

        let account = state.accounts.get(&1).unwrap();
        assert_eq!(account.total, 120.0);
        assert_eq!(account.held, 75.5);
        assert_eq!(account.available, 44.5);
    }

    #[test]
    fn test_negative_txs() {
        let mut state = State::default();
        assert_eq!(
            state.execute(Transaction {
                tx_type: TransactionType::Deposit { amount: -120.0 },
                client: 1,
                tx: 1,
            }),
            Err(TxError::InternalError)
        );

        assert_eq!(
            state.execute(Transaction {
                tx_type: TransactionType::Withdrawal { amount: -120.0 },
                client: 1,
                tx: 1,
            }),
            Err(TxError::InternalError)
        );
    }

    #[test]
    fn test_dispute_after_withdrawal() {
        let mut state = State::default();
        state
            .execute(Transaction {
                tx_type: TransactionType::Deposit { amount: 120.0 },
                client: 1,
                tx: 1,
            })
            .unwrap();

        state
            .execute(Transaction {
                tx_type: TransactionType::Withdrawal { amount: 120.0 },
                client: 1,
                tx: 2,
            })
            .unwrap();

        state
            .execute(Transaction {
                tx_type: TransactionType::Dispute,
                client: 1,
                tx: 1,
            })
            .unwrap();

        state
            .execute(Transaction {
                tx_type: TransactionType::Chargeback,
                client: 1,
                tx: 1,
            })
            .unwrap();

        let account = state.accounts.get(&1).unwrap();
        assert_eq!(account.total, -120.0);
        assert_eq!(account.held, 0.0);
        assert_eq!(account.available, -120.0);
    }

    #[test]
    fn test_account_lock() {
        let mut state = State::default();
        state
            .execute(Transaction {
                tx_type: TransactionType::Deposit { amount: 120.0 },
                client: 1,
                tx: 1,
            })
            .unwrap();

        state
            .execute(Transaction {
                tx_type: TransactionType::Dispute,
                client: 1,
                tx: 1,
            })
            .unwrap();

        state
            .execute(Transaction {
                tx_type: TransactionType::Chargeback,
                client: 1,
                tx: 1,
            })
            .unwrap();

        assert_eq!(
            state.execute(Transaction {
                tx_type: TransactionType::Deposit { amount: 120.0 },
                client: 1,
                tx: 1,
            }),
            Err(TxError::AccountLocked)
        );
    }
}
