use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as Base64Engine, Engine as _};
use borsh::BorshDeserialize;
use clap::Parser;
use std::{str::FromStr, sync::Arc};
use tracing::Level;
use url::Url;

use hyperlane_core::{ChainResult, Decode as _, HyperlaneDomain, HyperlaneMessage, H256, NativeToken};
use hyperlane_sealevel::{
    ConnectionConf, PriorityFeeOracleConfig, SealevelProvider,
};
use hyperlane_sealevel::rpc::fallback::SealevelFallbackRpcClient;
use hyperlane_sealevel::tx_submitter::config::TransactionSubmitterConfig;
// Import necessary items directly from the mailbox crate
use hyperlane_sealevel_mailbox::{
    accounts::{DispatchedMessageAccount, DISPATCHED_MESSAGE_DISCRIMINATOR},
    mailbox_dispatched_message_pda_seeds,
};

use solana_sdk::{account::Account, pubkey::Pubkey};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// RPC endpoint URL for the Solana node
    #[arg(long, default_value = "http://127.0.0.1:8899")]
    rpc_url: String,

    /// Mailbox program address (Base58)
    #[arg(long)]
    mailbox_address: String,

    /// Mailbox domain ID
    #[arg(long)]
    domain_id: u32,

    /// Nonce (sequence number) of the message to inspect
    #[arg(long)]
    nonce: u32,
}

// Helper function mimicking the logic from hyperlane-sealevel::account
async fn search_accounts_by_discriminator(
    provider: &SealevelProvider,
    program_id: &Pubkey,
    discriminator: &[u8; 8],
    nonce_bytes: &[u8],
    offset: usize,
    length: usize,
) -> ChainResult<Vec<(Pubkey, Account)>> {
    use hyperlane_core::ChainCommunicationError;
    use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
    use solana_client::rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType};
    use solana_sdk::commitment_config::CommitmentConfig;

    let target_message_account_bytes = &[discriminator, nonce_bytes].concat();
    let target_message_account_bytes = Base64Engine.encode(target_message_account_bytes);

    #[allow(deprecated)]
    let memcmp = RpcFilterType::Memcmp(Memcmp {
        // Mailbox account data has no `initialized` flag, start search from discriminator.
        offset: 0, // <-- Adjusted: Mailbox accounts don't start with initialized bool
        bytes: MemcmpEncodedBytes::Base64(target_message_account_bytes),
        encoding: None,
    });
    let config = RpcProgramAccountsConfig {
        filters: Some(vec![memcmp]),
        account_config: RpcAccountInfoConfig {
            encoding: Some(solana_account_decoder::UiAccountEncoding::Base64),
            data_slice: Some(solana_account_decoder::UiDataSliceConfig { offset, length }),
            commitment: Some(CommitmentConfig::finalized()),
            min_context_slot: None,
        },
        with_context: Some(false),
    };

    provider
        .rpc_client()
        .get_program_accounts_with_config(*program_id, config)
        .await
        .map_err(ChainCommunicationError::from_other) // Convert ClientError
}

// Helper function mimicking the logic from hyperlane-sealevel::account
fn search_and_validate_account<F>(
    accounts: Vec<(Pubkey, Account)>,
    mailbox_program_id: &Pubkey, // Need program ID here
    message_account_extractor: F,
) -> ChainResult<Pubkey>
where
    F: Fn(&Account) -> ChainResult<(Pubkey, u8)>, // Return pubkey and bump
{
    use hyperlane_core::ChainCommunicationError;

    for (pubkey, account) in accounts {
        let (unique_message_pubkey, _bump) = message_account_extractor(&account)?;
        // Re-derive the expected PDA using the *extracted* unique key and bump
        let (expected_pubkey, _derived_bump) = Pubkey::try_find_program_address(
            mailbox_dispatched_message_pda_seeds!(unique_message_pubkey),
            mailbox_program_id,
        )
        .ok_or_else(|| {
            ChainCommunicationError::from_other_str("Could not re-derive PDA address")
        })?;

        if expected_pubkey == pubkey {
            return Ok(pubkey); // Found the correct PDA
        }
    }

    Err(ChainCommunicationError::from_other_str(
        "Could not find valid storage PDA pubkey matching derivation",
    ))
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing (optional, but helpful for debugging)
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let args = Args::parse();

    let mailbox_pubkey = Pubkey::from_str(&args.mailbox_address)
        .context("Invalid mailbox program address provided")?;
    let _mailbox_h256 = H256::from_slice(&mailbox_pubkey.to_bytes()); // Keep for potential future use

    // --- Set up Provider ---
    // We need a ConnectionConf even if we don't use all parts for this simple task
    let dummy_conf = ConnectionConf {
        urls: vec![Url::parse(&args.rpc_url)?], // Use the provided RPC URL
        operation_batch: Default::default(),
        native_token: NativeToken {
            // Define SOL as native token (decimals important for potential balance checks)
            denom: "SOL".to_string(),
            decimals: 9,
        },
        priority_fee_oracle: PriorityFeeOracleConfig::default(), // Default needed
        transaction_submitter: TransactionSubmitterConfig::default(), // Default needed
    };
    let domain = HyperlaneDomain::Unknown {
        domain_id: args.domain_id,
        domain_name: "local_sealevel".to_string(),
        domain_type: hyperlane_core::HyperlaneDomainType::LocalTestChain,
        domain_protocol: hyperlane_core::HyperlaneDomainProtocol::Sealevel,
        domain_technical_stack: hyperlane_core::HyperlaneDomainTechnicalStack::Other,
    };
    let rpc_fallback =
        SealevelFallbackRpcClient::from_urls(None, dummy_conf.urls.clone(), Default::default());
    let provider = Arc::new(SealevelProvider::new(
        rpc_fallback,
        domain,
        &[], // No extra contract addresses needed here
        &dummy_conf,
    ));
    println!("Provider created for domain {}", args.domain_id);

    // --- Search for the Dispatched Message Account PDA ---
    println!(
        "Searching for dispatched message PDA for nonce {}...",
        args.nonce
    );
    let nonce_bytes = args.nonce.to_le_bytes();
    // Offset to skip discriminator (8 bytes), nonce (4 bytes), slot (8 bytes)
    // to reach the unique_message_pubkey field.
    let unique_dispatched_message_pubkey_offset = 8 + 4 + 8;
    let unique_dispatch_message_pubkey_length = 32; // the length of the `unique_message_pubkey` field

    // Use the helper function to find potential account candidates
    let accounts = search_accounts_by_discriminator(
        &provider,
        &mailbox_pubkey,
        DISPATCHED_MESSAGE_DISCRIMINATOR, // Use discriminator from mailbox crate
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
            args.nonce
        ));
    }

    // Closure to extract unique key from the *partial* data fetched by search_accounts_by_discriminator
    let dispatched_message_account_extractor = |account: &Account| -> ChainResult<(Pubkey, u8)> {
        // Data slice only contains the unique_message_pubkey here
        if account.data.len() != 32 {
            return Err(hyperlane_core::ChainCommunicationError::from_other_str(
                "Invalid data slice length received from search",
            ));
        }
        let unique_message_pubkey = Pubkey::new(&account.data);
        // We don't know the bump seed from the slice, return 0 as placeholder (it's not used in validation logic)
        Ok((unique_message_pubkey, 0))
    };

    let valid_pda_pubkey = search_and_validate_account(
        accounts,
        &mailbox_pubkey, // Pass program ID for derivation
        dispatched_message_account_extractor,
    )
    .context("Failed to find and validate the correct dispatched message PDA")?;

    println!("Validated PDA Address: {}", valid_pda_pubkey);

    // --- Fetch and Decode Full PDA Data ---
    println!("Fetching full account data for PDA...");
    let full_account_data = provider
        .rpc_client()
        .get_account_with_finalized_commitment(valid_pda_pubkey)
        .await?
        .data;
    println!("Fetched {} bytes of data.", full_account_data.len());

    // Deserialize the outer DispatchedMessage structure using the definition from the mailbox crate
    let dispatched_account =
        DispatchedMessageAccount::fetch(&mut &full_account_data[..])?.into_inner();

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
        hex::encode(&dispatched_account.encoded_message)
    );

    // Deserialize the inner HyperlaneMessage
    let message = HyperlaneMessage::read_from(&mut &dispatched_account.encoded_message[..])?;

    println!("\n--- Decoded Hyperlane Message ---");
    println!("  Version: {}", message.version);
    println!("  Nonce: {}", message.nonce);
    println!("  Origin Domain: {}", message.origin);
    println!("  Sender: 0x{}", hex::encode(message.sender));
    println!("  Destination Domain: {}", message.destination);
    println!("  Recipient: 0x{}", hex::encode(message.recipient));
    println!("  Body Length: {}", message.body.len());
    println!("  Body Hex: 0x{}", hex::encode(&message.body));

    // Try to decode body as UTF-8 string, if applicable
    if let Ok(body_str) = String::from_utf8(message.body.clone()) {
        // Check if it's printable or contains control characters
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