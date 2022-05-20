use std::collections::HashMap;

use client::LocalClientSync;
use solana_program_test::programs::spl_programs;
use solana_sdk::{
    account::Account, bpf_loader, pubkey::Pubkey, rent::Rent, signature::Keypair, signer::Signer,
    system_program,
};

pub mod client;

pub struct AccountConfig {
    pub lamports: Option<u64>,
    pub data: Vec<u8>,
    pub owner: Pubkey,
    pub executable: bool,
}

impl Default for AccountConfig {
    fn default() -> Self {
        Self {
            lamports: None,
            data: Vec::new(),
            owner: system_program::id(),
            executable: false,
        }
    }
}

pub struct EnvironmentGenesis {
    accounts: HashMap<Pubkey, Account>,
    address_labels: HashMap<Pubkey, String>,
    payer: Option<Keypair>,
}

impl EnvironmentGenesis {
    /// Creates a new [Self] with builtin items.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a payer keypair who will pay for the transaction.
    ///
    /// In local environment, a new account that holds huge amount of lamports will be added to the address.
    /// If omitted, a new keypair will be automatically generated when building the local environment.
    ///
    /// In remote environment, the existence of this account will be checked
    /// when the environment is built.
    pub fn add_payer(mut self, keypair: Keypair) -> Self {
        let pubkey = keypair.pubkey();
        assert!(self.payer.is_none());
        self.payer = Some(keypair);
        self.add_address_label("Payer", pubkey)
    }

    /// Adds an account to the initial account set.
    ///
    /// For local environment, these accounts will be added to the bank before running the PoC code.
    ///
    /// For remote environment, the existence of these accounts will be checked
    /// when the environment is built.
    pub fn add_account(mut self, address: Pubkey, config: AccountConfig) -> Self {
        assert!(
            self.accounts
                .insert(
                    address,
                    Account {
                        lamports: config.lamports.unwrap_or_else(|| Rent::default()
                            .minimum_balance(config.data.len())
                            .min(1)),
                        data: config.data,
                        owner: config.owner,
                        executable: config.executable,
                        rent_epoch: 0,
                    }
                )
                .is_none(),
            "Account added to the same address more than once"
        );
        self
    }

    /// Adds a program to the initial account set.
    pub fn add_program(mut self, address: Pubkey, data: Vec<u8>) -> Self {
        assert!(
            self.accounts
                .insert(
                    address,
                    Account {
                        lamports: Rent::default().minimum_balance(data.len()).min(1),
                        data,
                        owner: bpf_loader::ID,
                        executable: true,
                        rent_epoch: 0,
                    },
                )
                .is_none(),
            "Account added to the same address more than once"
        );
        self
    }

    /// Adds a new address label.
    pub fn add_address_label<S, P>(mut self, label: S, address: P) -> Self
    where
        S: ToString,
        P: TryInto<Pubkey>,
        <P as TryInto<Pubkey>>::Error: std::fmt::Debug,
    {
        self.address_labels.insert(
            address.try_into().expect("Invalid address"),
            label.to_string(),
        );
        self
    }

    /// Builds a [LocalClientSync] from the current configuration.
    pub fn build_local_sync(self) -> Environment<LocalClientSync> {
        LocalClientSync::new(self)
    }
}

impl Default for EnvironmentGenesis {
    fn default() -> Self {
        let mut this = Self {
            accounts: Default::default(),
            address_labels: Default::default(),
            payer: None,
        };

        for (addr, data) in spl_programs(&Rent::default()) {
            this.accounts.insert(addr, data.into());
        }

        this
            // Builtin Programs
            .add_address_label("System Program", "11111111111111111111111111111111")
            .add_address_label(
                "Native Loader",
                "NativeLoader1111111111111111111111111111111",
            )
            .add_address_label("BPF Loader", "BPFLoader2111111111111111111111111111111111")
            .add_address_label(
                "BPF Upgradeable Loader",
                "BPFLoaderUpgradeab1e11111111111111111111111",
            )
            // SPL Programs
            .add_address_label("SPL Token", "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")
            .add_address_label(
                "SPL Memo 1.0",
                "Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo",
            )
            .add_address_label(
                "SPL Memo 3.0",
                "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr",
            )
            .add_address_label(
                "SPL Associated Token",
                "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL",
            )
    }
}

pub struct Environment<C> {
    client: C,
    address_labels: HashMap<Pubkey, String>,
    payer: Keypair,
}
