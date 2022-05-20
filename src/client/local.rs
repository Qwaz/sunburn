use solana_runtime::bank::Bank;
use solana_sdk::{
    account::{Account, AccountSharedData},
    genesis_config::GenesisConfig,
    native_token::sol_to_lamports,
    signature::Keypair,
    signer::Signer,
    system_program,
};

use crate::{Environment, EnvironmentGenesis};

pub struct LocalClientSync {
    bank: Bank,
}

impl LocalClientSync {
    pub(crate) fn new(genesis: EnvironmentGenesis) -> Environment<Self> {
        let payer = match genesis.payer {
            Some(keypair) => keypair,
            None => Keypair::new(),
        };

        let mut accounts: Vec<_> = genesis
            .accounts
            .iter()
            .map(|(&address, account)| (address, AccountSharedData::from(account.clone())))
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
        }
    }
}
