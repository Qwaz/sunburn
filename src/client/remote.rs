use solana_client::{
    client_error::{ClientError as SolanaClientError, ClientErrorKind as SolanaClientErrorKind},
    rpc_client::RpcClient,
    rpc_custom_error::{
        JSON_RPC_SERVER_ERROR_SEND_TRANSACTION_PREFLIGHT_FAILURE,
        JSON_RPC_SERVER_ERROR_TRANSACTION_SIGNATURE_VERIFICATION_FAILURE,
    },
    rpc_request::{RpcError, RpcResponseErrorData},
};
use solana_sdk::{
    account::Account,
    account_info::IntoAccountInfo,
    commitment_config::CommitmentConfig,
    hash::Hash,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{Sysvar, SysvarId},
    transaction::{Transaction, TransactionError},
};
use solana_transaction_status::UiTransactionEncoding;

use super::{ClientError, ClientSync, TransactionDetails};
use crate::{Environment, EnvironmentGenesis};

pub struct RemoteClientSync {
    client: RpcClient,
}

/// Wrapper around `RpcClient::get_account_with_commitment`.
/// This methods panics when the provided account does not exist
/// and returns `Err` when the communication fails.
fn get_existing_account(
    client: &RpcClient,
    pubkey: &Pubkey,
) -> Result<Account, ClientError<SolanaClientError>> {
    Ok(client
        .get_account_with_commitment(&Rent::id(), CommitmentConfig::finalized())?
        .value
        .expect(&format!(
            "Account {} should exist in the remote environment",
            pubkey
        )))
}

impl RemoteClientSync {
    pub(crate) fn new(
        genesis: EnvironmentGenesis,
        url: String,
    ) -> Result<Environment<Self>, ClientError<SolanaClientError>> {
        let client = RpcClient::new(url);
        let mut rent_account_pair = (Rent::id(), get_existing_account(&client, &Rent::id())?);
        let rent = Rent::from_account_info(&rent_account_pair.into_account_info())
            .expect("Rent account data corruption");

        for account_key in genesis.accounts().keys() {
            // asserts existence of accounts defined in `EnvironmentGenesis`
            get_existing_account(&client, account_key)?;
        }

        let payer = genesis
            .payer
            .expect("Payer should be specified for remote client");

        // promote RpcClient into RemoteClientSync
        let client = RemoteClientSync { client };

        Ok(Environment {
            client,
            _address_labels: genesis.address_labels,
            payer,
            rent,
            log_config: genesis.log_config.unwrap_or_default(),
        })
    }
}

impl ClientSync for RemoteClientSync {
    type ChannelError = SolanaClientError;

    fn send_transaction(
        &mut self,
        transaction: Transaction,
    ) -> Result<TransactionDetails, ClientError<Self::ChannelError>> {
        let result = self.client.send_and_confirm_transaction(&transaction);

        // Translate back RPC failure into simulation failure
        match result {
            Ok(signature) => {
                let transaction_data = self
                    .client
                    .get_transaction(&signature, UiTransactionEncoding::Base64)?;

                // FIXME: Investigate if we ever get `None` case here
                let transaction_meta = transaction_data.transaction.meta.unwrap();
                let details = TransactionDetails {
                    log_messages: transaction_meta.log_messages.unwrap_or_default(),
                    // `UiTransactionStatusMeta` does not return # of units consumed
                    units_consumed: None,
                };

                match transaction_meta.err {
                    None => Ok(details),
                    Some(error) => Err(ClientError::FailedTransaction { error, details }),
                }
            }
            Err(mut err) => {
                if let SolanaClientErrorKind::RpcError(RpcError::RpcResponseError {
                    code,
                    data,
                    ..
                }) = &mut err.kind
                {
                    if *code == JSON_RPC_SERVER_ERROR_TRANSACTION_SIGNATURE_VERIFICATION_FAILURE {
                        return Err(ClientError::InvalidTransaction(
                            TransactionError::SignatureFailure,
                        ));
                    } else if *code == JSON_RPC_SERVER_ERROR_SEND_TRANSACTION_PREFLIGHT_FAILURE {
                        if let RpcResponseErrorData::SendTransactionPreflightFailure(
                            simulation_result,
                        ) = data
                        {
                            return Err(ClientError::FailedTransaction {
                                error: simulation_result.err.take().unwrap(),
                                details: TransactionDetails {
                                    log_messages: simulation_result.logs.take().unwrap_or_default(),
                                    units_consumed: simulation_result.units_consumed.take(),
                                },
                            });
                        }
                    }
                }

                Err(err.into())
            }
        }
    }

    fn latest_blockhash(&mut self) -> Result<Hash, Self::ChannelError> {
        todo!()
    }

    fn tick_beyond(&mut self, blockhash: Hash) -> Result<Hash, Self::ChannelError> {
        todo!()
    }

    fn get_account(&mut self, address: Pubkey) -> Result<Account, ClientError<Self::ChannelError>> {
        todo!()
    }
}
