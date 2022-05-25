mod local;
// TODO: mod remote;

use solana_banks_interface::TransactionSimulationDetails;
use solana_sdk::{
    account::{from_account, Account},
    hash::Hash,
    pubkey::Pubkey,
    sysvar::Sysvar,
    transaction::{Transaction, TransactionError},
};
use thiserror::Error;

pub use self::local::LocalClientSync;

// #[from] is deliberately avoided to prevent ambiguity.
// `TransactionSimulationDetails` is chosen as an intersection type of
// possible execution results.
#[derive(Debug, Error)]
pub enum ClientError<E: std::error::Error> {
    #[error("channel error: {0}")]
    ChannelError(#[source] E),
    #[error("invalid transaction: {:?}", 0)]
    InvalidTransaction(#[source] TransactionError),
    #[error("transaction failed to execute: {:?}", error)]
    FailedTransaction {
        error: TransactionError,
        details: TransactionSimulationDetails,
    },
    #[error("account not found: {}", 0)]
    AccountNotFound(Pubkey),
    #[error("account contains invalid data that cannot be deserialized")]
    InvalidAccountData,
}

pub trait ClientSync {
    type ChannelError: std::error::Error;

    fn send_transaction(
        &mut self,
        transaction: Transaction,
    ) -> Result<TransactionSimulationDetails, ClientError<Self::ChannelError>>;

    fn latest_blockhash(&mut self) -> Result<Hash, Self::ChannelError>;

    fn get_account(&mut self, address: Pubkey) -> Result<Account, ClientError<Self::ChannelError>>;

    fn get_sysvar<T: Sysvar>(&mut self) -> Result<T, ClientError<Self::ChannelError>> {
        self.get_account(T::id())
            .map(|account| from_account::<T, _>(&account).expect("Failed to deserialize sysvar"))
    }
}
