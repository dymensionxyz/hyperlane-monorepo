//! Single roundtrip command - reuses round_trip.rs logic for a single test run
//! Deposits from kaspa wallet to hub, then withdraws back to the same kaspa address

use super::hub_whale_pool::HubWhale;
use super::kaspa_whale_pool::KaspaWhale;
use super::key_cosmos::EasyHubKey;
use super::round_trip::{do_round_trip, TaskArgs, TaskResources};
use super::stats::RoundTripStats;
use super::util::{create_cosmos_provider, SOMPI_PER_KAS};
use crate::x::args::RoundtripCli;
use dym_kas_core::api::base::RateLimitConfig;
use dym_kas_core::api::client::HttpClient;
use dym_kas_core::wallet::{EasyKaspaWallet, EasyKaspaWalletArgs};
use eyre::Result;
use kaspa_wallet_core::prelude::Secret;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::sync::Mutex as AsyncMutex;
use tokio_util::sync::CancellationToken;
use tracing::info;

pub async fn do_roundtrip(cli: RoundtripCli) -> Result<RoundtripResult> {
    info!("starting roundtrip test");

    let kaspa_network = cli.bridge.parse_kaspa_network()?;
    let escrow_address = cli.bridge.parse_escrow_address()?;

    // Initialize Kaspa wallet
    info!("initializing kaspa wallet");
    let kaspa_wallet = EasyKaspaWallet::try_new(EasyKaspaWalletArgs {
        wallet_secret: cli.kaspa_wallet_secret.clone(),
        wrpc_url: cli.bridge.kaspa_wrpc_url.clone(),
        net: kaspa_network.clone(),
        storage_folder: cli.kaspa_wallet_dir.clone(),
    })
    .await?;

    let kaspa_secret = Secret::from(cli.kaspa_wallet_secret.clone());
    let kaspa_receive_addr = kaspa_wallet.wallet.account()?.receive_address()?;

    // Initialize Hub wallet
    let hub_key = EasyHubKey::from_hex(&cli.hub_priv_key);
    let hub_address = hub_key.signer().address_string.clone();

    // Print configuration
    println!("=== Kaspa Bridge Roundtrip Test ===");
    println!();
    println!("Configuration:");
    println!("  Kaspa wallet:  {}", kaspa_receive_addr);
    println!("  Hub wallet:    {}", hub_address);
    println!("  Escrow:        {}", escrow_address);
    println!(
        "  Deposit:       {} sompi ({:.2} KAS)",
        cli.bridge.deposit_amount,
        cli.bridge.deposit_amount as f64 / SOMPI_PER_KAS as f64
    );
    println!("  Network:       {}", cli.bridge.kaspa_network);
    println!("  Timeout:       {}s", cli.timeout);
    println!();

    // Create Hub provider
    let hub_provider = create_cosmos_provider(
        &hub_key,
        &cli.bridge.hub_rpc_url,
        &cli.bridge.hub_grpc_url,
        &cli.bridge.hub_chain_id,
        &cli.bridge.hub_prefix,
        &cli.bridge.hub_denom,
        cli.bridge.hub_decimals,
    )
    .await?;

    // Create REST client for Kaspa
    let kas_rest = HttpClient::new(
        cli.bridge.kaspa_rest_url.clone(),
        RateLimitConfig::default(),
    );

    // Build TaskArgs (reusing existing struct from round_trip.rs)
    let task_args = TaskArgs {
        domain_kas: cli.bridge.domain_kas,
        token_kas_placeholder: cli.bridge.token_kas_placeholder,
        domain_hub: cli.bridge.domain_hub,
        token_hub: cli.bridge.token_hub,
        escrow_address,
        deposit_amount: cli.bridge.deposit_amount,
        withdrawal_fee_pct: cli.bridge.withdrawal_fee_pct,
    };

    let task_resources = TaskResources {
        hub: hub_provider.clone(),
        args: task_args,
        kas_rest,
        kaspa_network,
    };

    // Create single-use "whale" wrappers to satisfy the round_trip interface
    let kaspa_whale = Arc::new(KaspaWhale {
        wallet: kaspa_wallet,
        secret: kaspa_secret,
        last_used: Mutex::new(Instant::now()),
        id: 0,
    });

    let hub_whale = Arc::new(HubWhale {
        provider: hub_provider,
        last_used: Mutex::new(Instant::now()),
        id: 0,
        tx_lock: AsyncMutex::new(()),
    });

    // Create a channel to receive stats updates
    let (tx, mut rx) = mpsc::channel::<RoundTripStats>(32);

    // Create cancellation token with timeout
    let cancel_token = CancellationToken::new();
    let cancel_clone = cancel_token.clone();

    // Spawn timeout task
    let timeout_secs = cli.timeout;
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(timeout_secs)).await;
        cancel_clone.cancel();
    });

    // Run the roundtrip using existing logic
    println!("[1/2] Deposit: KAS -> Hub");

    do_round_trip(task_resources, kaspa_whale, hub_whale, &tx, 0, cancel_token).await;

    // Drain stats channel to get final result
    drop(tx);
    let mut final_stats: Option<RoundTripStats> = None;
    while let Some(stats) = rx.recv().await {
        final_stats = Some(stats);
    }

    let result = match final_stats {
        Some(stats) => RoundtripResult::from_stats(stats),
        None => RoundtripResult::error("no stats received from roundtrip"),
    };

    result.print_summary();
    Ok(result)
}

/// Result of a roundtrip test
#[derive(Debug)]
pub struct RoundtripResult {
    pub deposit_tx_id: Option<String>,
    pub deposit_latency_ms: Option<u128>,
    pub deposit_error: Option<String>,
    pub withdrawal_tx_id: Option<String>,
    pub withdrawal_latency_ms: Option<u128>,
    pub withdrawal_error: Option<String>,
}

impl RoundtripResult {
    fn from_stats(stats: RoundTripStats) -> Self {
        let deposit_latency_ms = stats
            .deposit_credit_time_millis
            .and_then(|credit| stats.kaspa_deposit_tx_time_millis.map(|tx| credit - tx));

        let withdrawal_latency_ms = stats
            .withdraw_credit_time_millis
            .and_then(|credit| stats.hub_withdraw_tx_time_millis.map(|tx| credit - tx));

        Self {
            deposit_tx_id: stats.kaspa_deposit_tx_id.map(|id| id.to_string()),
            deposit_latency_ms,
            deposit_error: stats.deposit_error.or(stats.deposit_credit_error),
            withdrawal_tx_id: stats.hub_withdraw_tx_id,
            withdrawal_latency_ms,
            withdrawal_error: stats.withdrawal_error.or(stats.withdraw_credit_error),
        }
    }

    fn error(msg: &str) -> Self {
        Self {
            deposit_tx_id: None,
            deposit_latency_ms: None,
            deposit_error: Some(msg.to_string()),
            withdrawal_tx_id: None,
            withdrawal_latency_ms: None,
            withdrawal_error: None,
        }
    }

    fn format_duration(ms: u128) -> String {
        let total_secs = ms / 1000;
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        if mins > 0 {
            format!("{}min {}sec", mins, secs)
        } else {
            format!("{}sec", secs)
        }
    }

    pub fn print_summary(&self) {
        let deposit_status = if let Some(ref err) = self.deposit_error {
            format!("FAILED ({})", err)
        } else if let Some(latency) = self.deposit_latency_ms {
            format!("OK {}", Self::format_duration(latency))
        } else {
            "INCOMPLETE".to_string()
        };

        let withdrawal_status = if let Some(ref err) = self.withdrawal_error {
            format!("FAILED ({})", err)
        } else if let Some(latency) = self.withdrawal_latency_ms {
            format!("OK {}", Self::format_duration(latency))
        } else if self.deposit_error.is_some() {
            "SKIPPED (deposit failed)".to_string()
        } else {
            "INCOMPLETE".to_string()
        };

        let deposit_tx_suffix = self
            .deposit_tx_id
            .as_ref()
            .map(|tx| format!(" (tx: {})", tx))
            .unwrap_or_default();
        let withdrawal_tx_suffix = self
            .withdrawal_tx_id
            .as_ref()
            .map(|tx| format!(" (tx: {})", tx))
            .unwrap_or_default();

        println!();
        println!("=== Summary ===");
        println!(
            "Deposit:    KAS -> Hub  {}{}",
            deposit_status, deposit_tx_suffix
        );
        println!(
            "Withdrawal: Hub -> KAS  {}{}",
            withdrawal_status, withdrawal_tx_suffix
        );
    }

    pub fn is_success(&self) -> bool {
        self.deposit_error.is_none()
            && self.withdrawal_error.is_none()
            && self.deposit_latency_ms.is_some()
            && self.withdrawal_latency_ms.is_some()
    }
}
