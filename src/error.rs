#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TxError {
    #[error("Account doesnt have enough funds")]
    NotEnoughFunds,
    #[error("Requested transaction doesnt exist")]
    TxDoesntExist,
    #[error("Invalid transaction to dispute")]
    InvalidDispute,
    #[error("Cannot dispute tx that the client doesnt own.")]
    Unauthorized,
    #[error("Transaction is already under dispute.")]
    TxAlreadyDisputed,
    #[error("Transaction must be under dispute.")]
    TxNotUnderDispute,
    #[error("Internal math error.")]
    InternalError,
    #[error("Account is locked.")]
    AccountLocked,
}
