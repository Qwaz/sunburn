use solana_runtime::{
    bank::{Bank, TransactionExecutionResult},
    builtins::Builtins,
};
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

use super::{ClientError, ClientSync, TransactionDetails};
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

        let mut bank = Bank::new_for_tests(&genesis_config);

        // Add loaders
        macro_rules! add_builtin {
            ($b:expr) => {
                bank.add_builtin(&$b.0, &$b.1, $b.2)
            };
        }
        add_builtin!(solana_bpf_loader_program::solana_bpf_loader_deprecated_program!());
        add_builtin!(solana_bpf_loader_program::solana_bpf_loader_program!());
        add_builtin!(solana_bpf_loader_program::solana_bpf_loader_upgradeable_program!());

        let client = LocalClientSync { bank };

        Environment {
            client,
            _address_labels: genesis.address_labels,
            payer,
            rent,
        }
    }
}

fn convert_tx_result<E: std::error::Error>(
    tx_result: TransactionExecutionResult,
) -> Result<TransactionDetails, ClientError<E>> {
    match tx_result {
        TransactionExecutionResult::Executed { details, .. } => {
            let details_core = TransactionDetails {
                log_messages: details.log_messages.unwrap_or(Vec::new()),
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
    ) -> Result<TransactionDetails, ClientError<Self::ChannelError>> {
        let txs = vec![VersionedTransaction::from(transaction)];
        let batch = self
            .bank
            .prepare_entry_batch(txs)
            .map_err(ClientError::InvalidTransaction)?;

        let (mut tx_result, _) = self.bank.load_execute_and_commit_transactions(
            &batch,
            MAX_PROCESSING_AGE,
            false,
            false,
            true,
            &mut Default::default(),
        );

        convert_tx_result(tx_result.execution_results.pop().unwrap())
    }

    fn latest_blockhash(&mut self) -> Result<Hash, Self::ChannelError> {
        Ok(self.bank.last_blockhash())
    }

    fn get_account(&mut self, address: Pubkey) -> Result<Account, ClientError<Self::ChannelError>> {
        self.bank
            .get_account(&address)
            .map(|account| account.into())
            .ok_or(ClientError::AccountNotFound(address))
    }
}
