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
#![deny(warnings)] // Keep existing lints

use agent::Scraper;
use anyhow::{anyhow, Context, Result}; // Add Result and anyhow/Context
use base64::{engine::general_purpose::STANDARD as Base64Engine, Engine as _}; // Add base64
use borsh::BorshDeserialize; // Add borsh
use hyperlane_base::agent_main;
use std::{str::FromStr, sync::Arc}; // Add FromStr and Arc
use tracing::Level; // Add tracing::Level
use url::Url; // Add Url

// Scraper modules (keep them, they might be needed for type resolution)
mod agent;
mod conversions;
mod date_time;
mod db;
mod settings;
mod store;

// --- Imports specifically needed for the debug function ---
use hyperlane_core::{
    ChainCommunicationError, ChainResult, Decode as _, HyperlaneDomain, HyperlaneMessage,
};
use hyperlane_sealevel::{
    ConnectionConf, PriorityFeeOracleConfig,
    SealevelProvider,
};
use hyperlane_sealevel_mailbox::{
    accounts::{AccountData, DispatchedMessageAccount, DISPATCHED_MESSAGE_DISCRIMINATOR},
    mailbox_dispatched_message_pda_seeds,
};
use solana_sdk::{account::Account, pubkey::Pubkey};
// --- End imports for debug function ---

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Initialize minimal tracing for debug mode
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    println!("--- Running in Debug Mode ---");

    // --- Call your debug function ---
    // !!! REPLACE PLACEHOLDERS HERE !!!
    debug(
        "http://127.0.0.1:8899", // rpc_url
        "692KZJaoe2KRcD6uhCQDLLXnLNA5ZLnfvdqjE4aX9iu1", // mailbox_address_str
        13375,                               // domain_id
        0,                                   // nonce
    )
    .await

    // --- Comment out the original agent start ---
    // println!("Scraper agent starting up...");
    // agent_main::<Scraper>().await
}

// --- Your debug function (integrated) ---
async fn debug(
    rpc_url: &str,
    mailbox_address_str: &str,
    domain_id: u32,
    nonce: u32,
) -> Result<()> {
    // Note: Removed direct dependency on 'self' and Mailbox struct from your original snippet
    // Requires hyperlane-sealevel, hyperlane-core, hyperlane-sealevel-mailbox, solana_sdk, borsh, base64, anyhow, hex, url, tracing, tokio

    let mailbox_pubkey = Pubkey::from_str(mailbox_address_str)
        .with_context(|| format!("Invalid mailbox program address: {}", mailbox_address_str))?;

    // Set up Minimal Provider
    let dummy_conf = ConnectionConf {
        urls: vec![Url::parse(rpc_url).context("Invalid RPC URL")?],
        operation_batch: Default::default(),
        native_token: NativeToken {
            name: "DebugToken".to_string(),
            symbol: "DBG".to_string(),
            decimals: 9,
        },
        priority_fee_oracle: PriorityFeeOracleConfig::default(),
        transaction_submitter: TransactionSubmitterConfig::default(),
    };
    let domain = HyperlaneDomain::new_test_domain("debug_sealevel").with_id(domain_id);
    let rpc_fallback =
        SealevelFallbackRpcClient::from_urls(None, dummy_conf.urls.clone(), Default::default());
    let provider = Arc::new(SealevelProvider::new(
        rpc_fallback,
        domain,
        &[],
        &dummy_conf,
    ));
    println!(
        "Provider created for domain {} connected to {}",
        domain_id, rpc_url
    );

    // Search for the Dispatched Message Account PDA
    println!("Searching for dispatched message PDA for nonce {}...", nonce);
    let nonce_bytes = nonce.to_le_bytes();
    let unique_dispatched_message_pubkey_offset = 8 + 4 + 8;
    let unique_dispatch_message_pubkey_length = 32;

    let accounts = search_accounts_by_discriminator_minimal(
        &provider,
        &mailbox_pubkey,
        DISPATCHED_MESSAGE_DISCRIMINATOR,
        &nonce_bytes,
        unique_dispatched_message_pubkey_offset,
        unique_dispatch_message_pubkey_length,
    )
    .await
    .context("Failed to search for program accounts")?;

    println!("Found {} potential account(s). Validating...", accounts.len());
    if accounts.is_empty() {
        return Err(anyhow!(
            "No accounts found matching discriminator and nonce {}",
            nonce
        ));
    }

    let dispatched_message_account_extractor = |account: &Account| -> ChainResult<(Pubkey, u8)> {
        if account.data.len() != 32 {
            return Err(ChainCommunicationError::from_other_str(
                "Invalid data slice length received from search",
            ));
        }
        let unique_message_pubkey = Pubkey::new(&account.data);
        Ok((unique_message_pubkey, 0))
    };

    let valid_pda_pubkey = search_and_validate_account_minimal(
        accounts,
        &mailbox_pubkey,
        dispatched_message_account_extractor,
    )
    .context("Failed to find and validate the correct dispatched message PDA")?;

    println!("Validated PDA Address: {}", valid_pda_pubkey);

    // Fetch and Decode Full PDA Data
    println!("Fetching full account data for PDA...");
    let full_account_data = provider
        .rpc_client()
        .get_account_with_finalized_commitment(valid_pda_pubkey)
        .await?
        .data;
    println!("Fetched {} bytes of data.", full_account_data.len());

    let dispatched_account =
        AccountData::<hyperlane_sealevel_mailbox::accounts::DispatchedMessage>::fetch(
            &mut &full_account_data[..],
        )?
        .into_inner();

    println!("\n--- Dispatched Message Account Data ---");
    println!("  Nonce: {}", dispatched_account.nonce);
    println!("  Slot: {}", dispatched_account.slot);
    println!("  Unique Pubkey: {}", dispatched_account.unique_message_pubkey);
    println!(
        "  Encoded Message Length: {}",
        dispatched_account.encoded_message.len()
    );
    println!(
        "  Encoded Message Hex: 0x{}",
        hex::encode(&dispatched_account.encoded_message) // Requires `hex` crate dependency
    );

    // Deserialize the inner HyperlaneMessage
    let message = HyperlaneMessage::read_from(&mut &dispatched_account.encoded_message[..])?;

    println!("\n--- Decoded Hyperlane Message ---");
    println!("  Version: {}", message.version);
    println!("  Nonce: {}", message.nonce);
    println!("  Origin Domain: {}", message.origin);
    println!("  Sender: 0x{}", hex::encode(message.sender)); // Requires `hex` crate dependency
    println!("  Destination Domain: {}", message.destination);
    println!("  Recipient: 0x{}", hex::encode(message.recipient)); // Requires `hex` crate dependency
    println!("  Body Length: {}", message.body.len());
    println!("  Body Hex: 0x{}", hex::encode(&message.body)); // Requires `hex` crate dependency

    if let Ok(body_str) = String::from_utf8(message.body.clone()) {
        if body_str.chars().all(|c| !c.is_control() || c.is_whitespace()) {
            println!("  Body UTF-8: '{}'", body_str);
        } else {
            println!("  Body UTF-8: (contains non-printable characters)");
        }
    } else {
        println!("  Body UTF-8: (not valid UTF-8)");
    }

    Ok(())
}

// --- Helper functions copied/adapted from hyperlane-sealevel::account ---
// Minimal versions required by the debug function

/// Minimal version of search_accounts_by_discriminator
async fn search_accounts_by_discriminator_minimal(
    provider: &SealevelProvider,
    program_id: &Pubkey,
    discriminator: &[u8; 8],
    nonce_bytes: &[u8],
    offset: usize,
    length: usize,
) -> ChainResult<Vec<(Pubkey, Account)>> {
    use solana_account_decoder::{UiAccountEncoding, UiDataSliceConfig};
    use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
    use solana_client::rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType};
    use solana_sdk::commitment_config::CommitmentConfig;

    let target_message_account_bytes = &[discriminator, nonce_bytes].concat();
    let target_message_account_bytes = Base64Engine.encode(target_message_account_bytes);

    #[allow(deprecated)]
    let memcmp = RpcFilterType::Memcmp(Memcmp {
        offset: 0,
        bytes: MemcmpEncodedBytes::Base64(target_message_account_bytes),
        encoding: None,
    });
    let config = RpcProgramAccountsConfig {
        filters: Some(vec![memcmp]),
        account_config: RpcAccountInfoConfig {
            encoding: Some(UiAccountEncoding::Base64),
            data_slice: Some(UiDataSliceConfig { offset, length }),
            commitment: Some(CommitmentConfig::finalized()),
            min_context_slot: None,
        },
        with_context: Some(false),
    };
    provider
        .rpc_client()
        .get_program_accounts_with_config(*program_id, config)
        .await
        .map_err(ChainCommunicationError::from_other)
}

/// Minimal version of search_and_validate_account
fn search_and_validate_account_minimal<F>(
    accounts: Vec<(Pubkey, Account)>,
    mailbox_program_id: &Pubkey,
    message_account_extractor: F,
) -> ChainResult<Pubkey>
where
    F: Fn(&Account) -> ChainResult<(Pubkey, u8)>,
{
    for (pubkey, account) in accounts {
        let (unique_message_pubkey, _bump) = message_account_extractor(&account)?;
        let (expected_pubkey, _derived_bump) = Pubkey::try_find_program_address(
            mailbox_dispatched_message_pda_seeds!(unique_message_pubkey),
            mailbox_program_id,
        )
        .ok_or_else(|| {
            ChainCommunicationError::from_other_str("Could not re-derive PDA address")
        })?;

        if expected_pubkey == pubkey {
            return Ok(pubkey);
        }
    }
    Err(ChainCommunicationError::from_other_str(
        "Could not find valid storage PDA pubkey matching derivation",
    ))
}