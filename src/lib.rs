use std::collections::HashMap;

use client::{ClientError, ClientSync, LocalClientSync};
use solana_program_test::programs::spl_programs;
use solana_sdk::{
    account::{Account, AccountSharedData, ReadableAccount},
    bpf_loader,
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    hash::Hash,
    instruction::Instruction,
    loader_instruction,
    message::Message,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    signature::Keypair,
    signer::Signer,
    system_instruction, system_program,
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address;

pub mod client;

#[derive(Clone)]
pub struct AccountConfig {
    /// Lamports to store in this account.
    /// If omitted, it will be automatically set to minimum rent-exempt amount.
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

impl AccountConfig {
    pub fn to_account(self, rent: &Rent) -> AccountSharedData {
        Account {
            lamports: self
                .lamports
                .unwrap_or_else(|| rent.minimum_balance(self.data.len()).min(1)),
            data: self.data,
            owner: self.owner,
            executable: self.executable,
            rent_epoch: 0,
        }
        .into()
    }
}

pub struct EnvironmentGenesis {
    accounts: HashMap<Pubkey, AccountConfig>,
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
            self.accounts.insert(address, config).is_none(),
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
                    AccountConfig {
                        data,
                        owner: bpf_loader::ID,
                        executable: true,
                        ..Default::default()
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
        solana_logger::setup_with_default("");
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

        for (addr, account) in spl_programs(&Rent::default()) {
            let account: Account = account.into();
            this.accounts.insert(
                addr,
                AccountConfig {
                    // Rent amount is calculated later in `build()`
                    lamports: None,
                    data: account.data,
                    owner: account.owner,
                    executable: account.executable,
                },
            );
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
    _address_labels: HashMap<Pubkey, String>,
    payer: Keypair,
    /// Cached [Rent] information
    rent: Rent,
}

fn instructions_to_tx(
    payer: &Keypair,
    latest_blockhash: Hash,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> Transaction {
    let mut signers_vec = vec![payer];
    signers_vec.extend_from_slice(signers);

    let message = Message::new(instructions, Some(&payer.pubkey()));
    Transaction::new(&signers_vec, message, latest_blockhash)
}

type ClientErrorSync<C> = client::ClientError<<C as ClientSync>::ChannelError>;

impl<C> Environment<C> {
    pub fn payer(&self) -> &Keypair {
        &self.payer
    }

    pub fn rent(&self) -> Rent {
        self.rent
    }

    pub fn rent_exemption_amount(&self, data_len: usize) -> u64 {
        self.rent.minimum_balance(data_len).max(1)
    }
}

impl<C: ClientSync> Environment<C> {
    /// Executes provided instructions as a transaction and returns the result.
    pub fn run_instructions(
        &mut self,
        instructions: &[Instruction],
        signers: &[&Keypair],
    ) -> Result<(), ClientErrorSync<C>> {
        let blockhash = self
            .client
            .latest_blockhash()
            .map_err(ClientError::ChannelError)?;
        let transaction = instructions_to_tx(&self.payer, blockhash, instructions, signers);
        self.client.send_transaction(transaction)?;
        Ok(())
    }

    /// Runs a single instruction as a transaction and returns the result.
    pub fn run_instruction(
        &mut self,
        instruction: Instruction,
        signers: &[&Keypair],
    ) -> Result<(), ClientErrorSync<C>> {
        self.run_instructions(&[instruction], signers)?;
        Ok(())
    }

    /// Gets account information at the given address.
    pub fn get_account(&mut self, address: Pubkey) -> Result<Account, ClientErrorSync<C>> {
        self.client.get_account(address)
    }

    /// Gets how much token does this token account hold.
    pub fn token_balance(&mut self, token_account: Pubkey) -> Result<u64, ClientErrorSync<C>> {
        let account = self.client.get_account(token_account)?;
        Ok(spl_token::state::Account::unpack_unchecked(&account.data())
            .map_err(|_| ClientError::InvalidAccountData)?
            .amount)
    }

    /// Creates a new token mint using the provided keypair.
    pub fn create_token_mint(
        &mut self,
        mint: &Keypair,
        authority: Pubkey,
        freeze_authority: Option<Pubkey>,
        decimals: u8,
    ) -> Result<(), ClientErrorSync<C>> {
        self.run_instructions(
            &[
                system_instruction::create_account(
                    &self.payer.pubkey(),
                    &mint.pubkey(),
                    self.rent.minimum_balance(spl_token::state::Mint::LEN),
                    spl_token::state::Mint::LEN as u64,
                    &spl_token::ID,
                ),
                spl_token::instruction::initialize_mint(
                    &spl_token::ID,
                    &mint.pubkey(),
                    &authority,
                    freeze_authority.as_ref(),
                    decimals,
                )
                .unwrap(),
            ],
            &[mint],
        )?;
        Ok(())
    }

    /// Mints tokens to `recipient` token account with the token authority keypair.
    pub fn mint_tokens(
        &mut self,
        mint: Pubkey,
        authority: &Keypair,
        recipient: Pubkey,
        amount: u64,
    ) -> Result<(), ClientErrorSync<C>> {
        self.run_instruction(
            spl_token::instruction::mint_to(
                &spl_token::ID,
                &mint,
                &recipient,
                &authority.pubkey(),
                &[],
                amount,
            )
            .unwrap(),
            &[authority],
        )?;
        Ok(())
    }

    /// Creates a token account using the associated token account scheme.
    pub fn create_associated_token_account(
        &mut self,
        owner: Pubkey,
        mint: Pubkey,
    ) -> Result<Pubkey, ClientErrorSync<C>> {
        // We need this deprecated version of API for our target version of Solana
        self.run_instruction(
            #[allow(deprecated)]
            spl_associated_token_account::create_associated_token_account(
                &self.payer.pubkey(),
                &owner,
                &mint,
            ),
            &[],
        )?;
        Ok(get_associated_token_address(&owner, &mint))
    }

    /// Creates an account with a data field.
    /// The account is required to be empty and will be owned by bpf_loader afterwards.
    pub fn create_account_with_data(
        &mut self,
        account: &Keypair,
        data: &[u8],
    ) -> Result<(), ClientErrorSync<C>> {
        self.run_instruction(
            system_instruction::create_account(
                &self.payer.pubkey(),
                &account.pubkey(),
                self.rent.minimum_balance(data.len()),
                data.len() as u64,
                &bpf_loader::id(),
            ),
            &[account],
        )?;

        let mut offset = 0usize;
        for chunk in data.chunks(900) {
            self.run_instruction(
                loader_instruction::write(
                    &account.pubkey(),
                    &bpf_loader::id(),
                    offset as u32,
                    chunk.to_vec(),
                ),
                &[account],
            )?;
            offset += chunk.len();
        }

        Ok(())
    }

    /// Create an executable account using a given keypair.
    pub fn deploy_program(
        &mut self,
        program_account: &Keypair,
        data: &[u8],
    ) -> Result<(), ClientErrorSync<C>> {
        self.create_account_with_data(program_account, data)?;
        self.run_instruction(
            loader_instruction::finalize(&program_account.pubkey(), &bpf_loader::id()),
            &[program_account],
        )?;

        Ok(())
    }

    /// Deploys an upgradeable program and returns its ProgramData address
    pub fn deploy_upgradeable_program(
        &mut self,
        program_account: &Keypair,
        buffer_account: &Keypair,
        authority_account: &Keypair,
        data: &[u8],
        compact: bool,
    ) -> Result<Pubkey, ClientErrorSync<C>> {
        let (programdata_address, _) = Pubkey::find_program_address(
            &[&program_account.pubkey().to_bytes()],
            &bpf_loader_upgradeable::ID,
        );

        let program_max_size = if compact { data.len() } else { data.len() * 2 };

        let buffer_balance = self
            .rent
            .minimum_balance(UpgradeableLoaderState::programdata_len(program_max_size).unwrap());
        self.run_instructions(
            &bpf_loader_upgradeable::create_buffer(
                &self.payer.pubkey(),
                &buffer_account.pubkey(),
                &authority_account.pubkey(),
                buffer_balance,
                program_max_size,
            )
            .unwrap(),
            &[buffer_account],
        )?;

        let mut offset = 0usize;
        for chunk in data.chunks(900) {
            self.run_instruction(
                bpf_loader_upgradeable::write(
                    &buffer_account.pubkey(),
                    &authority_account.pubkey(),
                    offset as u32,
                    chunk.to_vec(),
                ),
                &[authority_account],
            )?;
            offset += chunk.len();
        }

        self.run_instructions(
            &bpf_loader_upgradeable::deploy_with_max_program_len(
                &self.payer.pubkey(),
                &program_account.pubkey(),
                &buffer_account.pubkey(),
                &authority_account.pubkey(),
                self.rent
                    .minimum_balance(UpgradeableLoaderState::program_len().unwrap()),
                program_max_size,
            )
            .unwrap(),
            &[program_account, authority_account],
        )?;

        Ok(programdata_address)
    }
}
