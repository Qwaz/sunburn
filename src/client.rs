pub mod local;
pub mod remote;

pub use local::LocalClientSync;
use solana_sdk::{
    account::{from_account, Account},
    hash::Hash,
    pubkey::Pubkey,
    sysvar::Sysvar,
    transaction::{Transaction, TransactionError},
};
use thiserror::Error;

/// Generalized struct to represent the essence of
/// `solana_banks_interface::TransactionSimulationDetails`
/// and `solana_transaction_status::UiTransactionStatusMetaCopy`.
#[derive(Clone, Debug)]
pub struct TransactionDetails {
    pub log_messages: Vec<String>,
    /// Consumed amount of computation unit.
    /// Might be `None` for successfully executed remote transactions.
    pub units_consumed: Option<u64>,
}

#[derive(Debug, Error)]
pub enum ClientError<E: std::error::Error> {
    #[error("channel error: {0}")]
    ChannelError(#[from] E),
    /// An error that represents an invalid transaction
    /// that is invalid and not executed.
    #[error("invalid transaction: {}", 0)]
    InvalidTransaction(#[source] TransactionError),
    #[error("transaction failed to execute: {:?}", error)]
    /// An error that represents a transaction that was executed and failed.
    /// This includes a simulation failure in preflight check.
    FailedTransaction {
        error: TransactionError,
        details: TransactionDetails,
    },
    #[error("account not found: {}", 0)]
    AccountNotFound(Pubkey),
    #[error("account {} contains invalid data that cannot be deserialized", 0)]
    InvalidAccountData(Pubkey),
}

pub trait ClientSync {
    type ChannelError: std::error::Error;

    fn send_transaction(
        &mut self,
        transaction: Transaction,
    ) -> Result<TransactionDetails, ClientError<Self::ChannelError>>;

    fn latest_blockhash(&mut self) -> Result<Hash, Self::ChannelError>;

    fn tick_beyond(&mut self, blockhash: Hash) -> Result<Hash, Self::ChannelError>;

    fn get_account(&mut self, address: Pubkey) -> Result<Account, ClientError<Self::ChannelError>>;

    fn get_sysvar<T: Sysvar>(&mut self) -> Result<T, ClientError<Self::ChannelError>> {
        self.get_account(T::id()).and_then(|account| {
            from_account::<T, _>(&account).ok_or(ClientError::InvalidAccountData(T::id()))
        })
    }
}
