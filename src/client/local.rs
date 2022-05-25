use solana_banks_interface::TransactionSimulationDetails;
use solana_runtime::bank::{Bank, TransactionExecutionResult};
use solana_sdk::{
    account::Account,
    clock::MAX_PROCESSING_AGE,
    genesis_config::GenesisConfig,
    hash::Hash,
    native_token::sol_to_lamports,
    pubkey::Pubkey,
    rent::Rent,
    signature::Keypair,
    signer::Signer,
    system_program,
    transaction::{Transaction, VersionedTransaction},
};

use super::{ClientError, ClientSync};
use crate::{Environment, EnvironmentGenesis};

pub struct LocalClientSync {
    bank: Bank,
}

impl LocalClientSync {
    pub(crate) fn new(genesis: EnvironmentGenesis) -> Environment<Self> {
        let rent = Rent::default();

        let payer = match genesis.payer {
            Some(keypair) => keypair,
            None => Keypair::new(),
        };

        let mut accounts: Vec<_> = genesis
            .accounts
            .iter()
            .map(|(&address, account_config)| (address, account_config.clone().to_account(&rent)))
            .collect();

        accounts.push((
            payer.pubkey(),
            Account {
                lamports: sol_to_lamports(1_000_000_000.0),
                data: Default::default(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            }
            .into(),
        ));

        let genesis_config = GenesisConfig::new(&accounts, &[]);

        let bank = Bank::new_for_tests(&genesis_config);
        let client = LocalClientSync { bank };

        Environment {
            client,
            address_labels: genesis.address_labels,
            payer,
            rent,
        }
    }
}

fn convert_tx_result<E: std::error::Error>(
    tx_result: TransactionExecutionResult,
) -> Result<TransactionSimulationDetails, ClientError<E>> {
    match tx_result {
        TransactionExecutionResult::Executed { details, .. } => {
            let details_core = TransactionSimulationDetails {
                logs: details.log_messages.unwrap_or(Vec::new()),
                units_consumed: details.executed_units,
            };
            match details.status {
                Ok(()) => Ok(details_core),
                Err(error) => Err(ClientError::FailedTransaction {
                    error,
                    details: details_core,
                }),
            }
        }
        TransactionExecutionResult::NotExecuted(error) => {
            Err(ClientError::InvalidTransaction(error))
        }
    }
}

impl ClientSync for LocalClientSync {
    // Switch to ! type when it is stabilized
    type ChannelError = std::convert::Infallible;

    fn send_transaction(
        &mut self,
        transaction: Transaction,
    ) -> Result<TransactionSimulationDetails, ClientError<Self::ChannelError>> {
        let txs = vec![VersionedTransaction::from(transaction)];
        let batch = self
            .bank
            .prepare_entry_batch(txs)
            .map_err(ClientError::InvalidTransaction)?;

        let (tx_result, _) = self.bank.load_execute_and_commit_transactions(
            &batch,
            MAX_PROCESSING_AGE,
            false,
            false,
            true,
            &mut Default::default(),
        );

        convert_tx_result(tx_result.execution_results[0])
    }

    fn latest_blockhash(&mut self) -> Result<Hash, Self::ChannelError> {
        Ok(self.bank.last_blockhash())
    }

    fn get_account(&mut self, address: Pubkey) -> Result<Option<Account>, Self::ChannelError> {
        Ok(self
            .bank
            .get_account(&address)
            .map(|account| account.into()))
    }
}
