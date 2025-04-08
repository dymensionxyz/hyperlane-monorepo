//! The message explorer scraper is responsible for building and maintaining a
//! relational database of the Hyperlane state across blockchains to empower us and
//! our users to trace and debug messages and other system state.
//!
//! Information scrapped is predominately recoverable simply be re-scraping the
//! blockchains, however, they may be some additional "enrichment" which is only
//! practically discoverable at the time it was recorded. This additional
//! information is not critical to the functioning of the system.
//!
//! One scraper instance is run per chain and together they will be able to
//! piece together the full hyperlane system state in the relational database.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use agent::Scraper;
use eyre::Result;
use hyperlane_base::agent_main;

mod agent;
mod conversions;
mod date_time;
mod db;
mod settings;
mod store;

use hyperlane_sealevel::{
    account::search_accounts_by_discriminator, ConnectionConf, NativeToken,
    PriorityFeeOracleConfig, SealevelFallbackRpcClient, SealevelProvider,
    TransactionSubmitterConfig,
};

use hyperlane_core::{ChainCommunicationError, HyperlaneDomain, HyperlaneMessage};
use hyperlane_sealevel_mailbox::accounts::DispatchedMessageAccount;
use solana_sdk::pubkey::Pubkey;
use crate::account::search_and_validate_account;


#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Logging is not initialised at this point, so, using `println!`
    println!("Scraper agent starting up...");

    // agent_main::<Scraper>().await;

    Ok(())
}

async fn debug() -> Result<()> {
    let provider = SealevelProvider::new(
        SealevelFallbackRpcClient::new(rpc_url),
        HyperlaneDomain::new(domain_id),
        &contract_addresses,
        &conf,
    );

    let nonce_bytes = nonce.to_le_bytes();
    let unique_dispatched_message_pubkey_offset = 1 + 8 + 4 + 8; // the offset to get the `unique_message_pubkey` field
    let unique_dispatch_message_pubkey_length = 32; // the length of the `unique_message_pubkey` fiel
    let discriminator = hyperlane_sealevel_mailbox::accounts::DISPATCHED_MESSAGE_DISCRIMINATOR;

    let res = search_accounts_by_discriminator(
        &provider,
        &program_id,
        &discriminator,
        &nonce_bytes,
        unique_dispatched_message_pubkey_offset,
        unique_dispatch_message_pubkey_length,
    )
    .await?;

    let valid_message_storage_pda_pubkey =
        search_and_validate_account(accounts, |account| self.dispatched_message_account(account))?;

    let mailbox = Mailbox::new(provider, tx_submitter, locator, conf, advanced_log_meta);
    let account = mailbox
        .get_provider()
        .rpc_client()
        .get_account_with_finalized_commitment(valid_message_storage_pda_pubkey)
        .await?;

    let dispatched_message_account = DispatchedMessageAccount::fetch(&mut account.data.as_ref())
        .map_err(ChainCommunicationError::from_other)?
        .into_inner();
    let hyperlane_message =
        HyperlaneMessage::read_from(&mut &dispatched_message_account.encoded_message[..])?;
}
