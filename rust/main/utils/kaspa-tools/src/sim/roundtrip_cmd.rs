//! Single roundtrip test command
//! Deposits from kaspa wallet to hub wallet, then withdraws back to the same kaspa wallet

use crate::x::args::RoundtripCli;
use crate::x;
use corelib::api::base::RateLimitConfig;
use corelib::api::client::HttpClient;
use corelib::user::payload::make_deposit_payload_easy;
use corelib::wallet::{EasyKaspaWallet, EasyKaspaWalletArgs, Network};
use cosmos_sdk_proto::cosmos::base::v1beta1::Coin;
use cosmrs::Any;
use eyre::Result;
use hyperlane_core::config::OpSubmissionConfig;
use hyperlane_core::{ContractLocator, HyperlaneDomain, KnownHyperlaneDomain, NativeToken, H256, U256};
use hyperlane_cosmos::RawCosmosAmount;
use hyperlane_cosmos::{native::ModuleQueryClient, ConnectionConf as CosmosConnectionConf, CosmosProvider};
use hyperlane_cosmos_rs::hyperlane::warp::v1::MsgRemoteTransfer;
use hyperlane_cosmos_rs::prost::{Message, Name};
use hyperlane_metric::prometheus_metric::PrometheusClientMetrics;
use kaspa_addresses::Address;
use kaspa_wallet_core::prelude::Secret;
use std::io::{self, Write};
use std::time::Duration;
use url::Url;

use super::key_cosmos::EasyHubKey;

/// Result of a roundtrip test
#[derive(Debug)]
pub struct RoundtripResult {
    pub deposit_tx_id: Option<String>,
    pub deposit_time_ms: Option<u128>,
    pub deposit_credit_time_ms: Option<u128>,
    pub deposit_error: Option<String>,
    pub withdrawal_tx_id: Option<String>,
    pub withdrawal_time_ms: Option<u128>,
    pub withdrawal_credit_time_ms: Option<u128>,
    pub withdrawal_error: Option<String>,
}

impl RoundtripResult {
    pub fn new() -> Self {
        Self {
            deposit_tx_id: None,
            deposit_time_ms: None,
            deposit_credit_time_ms: None,
            deposit_error: None,
            withdrawal_tx_id: None,
            withdrawal_time_ms: None,
            withdrawal_credit_time_ms: None,
            withdrawal_error: None,
        }
    }

    pub fn deposit_latency_ms(&self) -> Option<u128> {
        match (self.deposit_time_ms, self.deposit_credit_time_ms) {
            (Some(start), Some(end)) => Some(end - start),
            _ => None,
        }
    }

    pub fn withdrawal_latency_ms(&self) -> Option<u128> {
        match (self.withdrawal_time_ms, self.withdrawal_credit_time_ms) {
            (Some(start), Some(end)) => Some(end - start),
            _ => None,
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
        // Build deposit status
        let deposit_status = if let Some(ref err) = self.deposit_error {
            format!("FAILED ({})", err)
        } else if let Some(latency) = self.deposit_latency_ms() {
            format!("OK {}", Self::format_duration(latency))
        } else {
            "INCOMPLETE".to_string()
        };

        // Build withdrawal status
        let withdrawal_status = if let Some(ref err) = self.withdrawal_error {
            format!("FAILED ({})", err)
        } else if let Some(latency) = self.withdrawal_latency_ms() {
            format!("OK {}", Self::format_duration(latency))
        } else if self.deposit_error.is_some() {
            "SKIPPED (deposit failed)".to_string()
        } else {
            "INCOMPLETE".to_string()
        };

        // Build transaction hash suffixes
        let deposit_tx_suffix = self
            .deposit_tx_id
            .as_ref()
            .map(|tx| format!(" ( tx: {} )", tx))
            .unwrap_or_default();
        let withdrawal_tx_suffix = self
            .withdrawal_tx_id
            .as_ref()
            .map(|tx| format!(" ( tx: {} )", tx))
            .unwrap_or_default();

        println!();
        println!("Deposit: KAS > DYM {}{}", deposit_status, deposit_tx_suffix);
        println!("Withdrawal: DYM > KAS {}{}", withdrawal_status, withdrawal_tx_suffix);
        let _ = io::stdout().flush();
    }

    pub fn is_success(&self) -> bool {
        self.deposit_error.is_none()
            && self.withdrawal_error.is_none()
            && self.deposit_credit_time_ms.is_some()
            && self.withdrawal_credit_time_ms.is_some()
    }
}

fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

pub async fn do_roundtrip(cli: RoundtripCli) -> Result<RoundtripResult> {
    println!("Starting roundtrip...");
    let _ = io::stdout().flush();

    let mut result = RoundtripResult::new();

    // Parse network
    let kaspa_network = match cli.kaspa_network.to_lowercase().as_str() {
        "testnet" => Network::KaspaTest10,
        "mainnet" => Network::KaspaMainnet,
        _ => return Err(eyre::eyre!("invalid kaspa network: {}", cli.kaspa_network)),
    };

    let escrow_address = Address::try_from(cli.escrow_address.clone())?;

    println!("=== Kaspa Bridge Roundtrip Test ===");
    println!();
    let _ = io::stdout().flush();

    // Initialize Kaspa wallet
    println!("Initializing Kaspa wallet...");
    let _ = io::stdout().flush();
    let kaspa_wallet = EasyKaspaWallet::try_new(EasyKaspaWalletArgs {
        wallet_secret: cli.kaspa_wallet_secret.clone(),
        wrpc_url: cli.kaspa_wrpc_url.clone(),
        net: kaspa_network.clone(),
        storage_folder: cli.kaspa_wallet_dir.clone(),
    })
    .await?;

    let kaspa_secret = Secret::from(cli.kaspa_wallet_secret.clone());
    let kaspa_receive_addr = kaspa_wallet.wallet.account()?.receive_address()?;

    // Initialize Hub whale
    let hub_key = EasyHubKey::from_hex(&cli.hub_whale_priv_key);
    let hub_signer = hub_key.signer();
    let hub_address = hub_signer.address_string.clone();

    // Print configuration
    println!("Configuration:");
    println!("  Kaspa wallet:  {}", kaspa_receive_addr);
    println!("  Hub wallet:    {}", hub_address);
    println!("  Escrow:        {}", escrow_address);
    println!("  Deposit:       {} sompi ({:.2} KAS)", cli.deposit_amount, cli.deposit_amount as f64 / 100_000_000.0);
    println!("  Network:       {}", cli.kaspa_network);
    println!("  Timeout:       {}s", cli.timeout);
    println!();
    let _ = io::stdout().flush();

    // Create Hub provider
    let hub_provider = create_cosmos_provider(
        &hub_key,
        &cli.hub_rpc_url,
        &cli.hub_grpc_url,
        &cli.hub_chain_id,
        &cli.hub_prefix,
        &cli.hub_denom,
        cli.hub_decimals,
    )
    .await?;

    // Create REST client for Kaspa
    let kas_rest = HttpClient::new(cli.kaspa_rest_url.clone(), RateLimitConfig::default());

    // Hub token denom
    let token_hub_str = format!("0x{}", hex::encode(cli.token_hub.as_bytes()));
    let hub_denom = format!("hyperlane/{}", token_hub_str);

    // Calculate withdrawal amount (deposit - fee)
    let withdrawal_amount = (cli.deposit_amount as f64 / (1.0 + cli.withdrawal_fee_pct)) as u64 - 1;
    let fee_amount = cli.deposit_amount - withdrawal_amount;

    // ========== PHASE 1: Deposit (Kaspa -> Hub) ==========
    println!("[1/2] Deposit: KAS -> Hub");
    println!("  Amount: {} sompi -> {}", cli.deposit_amount, hub_address);
    let _ = io::stdout().flush();

    // Get initial Hub balance before deposit
    let initial_hub_balance = hub_provider
        .rpc()
        .get_balance_denom(hub_address.clone(), hub_denom.clone())
        .await
        .unwrap_or(U256::zero());
    println!("  Initial Hub balance: {}", initial_hub_balance);
    let _ = io::stdout().flush();

    // Build deposit payload
    let payload = make_deposit_payload_easy(
        cli.domain_kas,
        cli.token_kas_placeholder,
        cli.domain_hub,
        cli.token_hub,
        cli.deposit_amount,
        &hub_signer,
    );

    // Execute deposit
    let deposit_result = corelib::user::deposit::deposit_with_payload(
        &kaspa_wallet.wallet,
        &kaspa_secret,
        escrow_address.clone(),
        cli.deposit_amount,
        payload,
    )
    .await;

    match deposit_result {
        Ok(tx_id) => {
            result.deposit_tx_id = Some(tx_id.to_string());
            result.deposit_time_ms = Some(now_millis());
            println!("  TX submitted: {}", tx_id);
            let _ = io::stdout().flush();
        }
        Err(e) => {
            result.deposit_error = Some(e.to_string());
            eprintln!("  ERROR: {}", e);
            let _ = io::stderr().flush();
            result.print_summary();
            return Ok(result);
        }
    }

    // Wait for deposit credit on Hub
    println!("  Waiting for credit on Hub (timeout: {}s)...", cli.timeout);
    let _ = io::stdout().flush();
    let deposit_credited = wait_for_hub_balance_increase(
        &hub_provider,
        &hub_address,
        &hub_denom,
        initial_hub_balance,
        cli.deposit_amount,
        Duration::from_secs(cli.timeout),
    )
    .await;

    if !deposit_credited {
        result.deposit_error = Some(format!("Deposit not credited within {}s timeout", cli.timeout));
        eprintln!("  ERROR: {}", result.deposit_error.as_ref().unwrap());
        result.print_summary();
        return Ok(result);
    }

    result.deposit_credit_time_ms = Some(now_millis());
    let deposit_latency = result.deposit_latency_ms().unwrap_or(0);
    println!("  Credited! Latency: {}", RoundtripResult::format_duration(deposit_latency));
    println!();
    let _ = io::stdout().flush();

    // ========== PHASE 2: Withdrawal (Hub -> Kaspa) ==========
    println!("[2/2] Withdrawal: Hub -> KAS");
    println!("  Amount: {} sompi -> {}", withdrawal_amount, kaspa_receive_addr);
    let _ = io::stdout().flush();

    // Get initial Kaspa balance
    let initial_kaspa_balance = kas_rest.get_balance_by_address(&kaspa_receive_addr.to_string()).await.unwrap_or(0);
    println!("  Initial Kaspa balance: {}", initial_kaspa_balance);
    let _ = io::stdout().flush();

    // Execute withdrawal
    let recipient_hl = x::addr::hl_recipient(&kaspa_receive_addr.to_string());

    let req = MsgRemoteTransfer {
        sender: hub_address.clone(),
        token_id: token_hub_str,
        destination_domain: cli.domain_kas,
        recipient: recipient_hl,
        amount: withdrawal_amount.to_string(),
        custom_hook_id: "".to_string(),
        gas_limit: "0".to_string(),
        max_fee: Some(Coin {
            denom: hub_denom.clone(),
            amount: fee_amount.to_string(),
        }),
        custom_hook_metadata: "".to_string(),
    };

    let msg = Any {
        type_url: MsgRemoteTransfer::type_url(),
        value: req.encode_to_vec(),
    };

    let rpc = hub_provider.rpc();
    let response = rpc.send(vec![msg], None).await;

    match response {
        Ok(resp) => {
            if resp.tx_result.code.is_ok() && resp.check_tx.code.is_ok() {
                let tx_hash = hex::encode_upper(resp.hash.as_bytes());
                result.withdrawal_tx_id = Some(tx_hash.clone());
                result.withdrawal_time_ms = Some(now_millis());
                println!("  TX submitted: {}", tx_hash);
                let _ = io::stdout().flush();
            } else {
                result.withdrawal_error = Some(format!(
                    "TX failed: tx_result={:?} check_tx={:?}",
                    resp.tx_result.code, resp.check_tx.code
                ));
                eprintln!("  ERROR: {}", result.withdrawal_error.as_ref().unwrap());
                let _ = io::stderr().flush();
                result.print_summary();
                return Ok(result);
            }
        }
        Err(e) => {
            result.withdrawal_error = Some(e.to_string());
            eprintln!("  ERROR: {}", e);
            let _ = io::stderr().flush();
            result.print_summary();
            return Ok(result);
        }
    }

    // Wait for withdrawal credit on Kaspa
    println!("  Waiting for credit on Kaspa (timeout: {}s)...", cli.timeout);
    let _ = io::stdout().flush();
    let withdrawal_credited = wait_for_kaspa_balance_increase(
        &kas_rest,
        &kaspa_receive_addr.to_string(),
        initial_kaspa_balance,
        withdrawal_amount,
        Duration::from_secs(cli.timeout),
    )
    .await;

    if !withdrawal_credited {
        result.withdrawal_error = Some(format!("Withdrawal not credited within {}s timeout", cli.timeout));
        eprintln!("  ERROR: {}", result.withdrawal_error.as_ref().unwrap());
        result.print_summary();
        return Ok(result);
    }

    result.withdrawal_credit_time_ms = Some(now_millis());
    let withdrawal_latency = result.withdrawal_latency_ms().unwrap_or(0);
    println!("  Credited! Latency: {}", RoundtripResult::format_duration(withdrawal_latency));

    result.print_summary();
    Ok(result)
}

async fn create_cosmos_provider(
    key: &EasyHubKey,
    rpc_url: &str,
    grpc_url: &str,
    chain_id: &str,
    prefix: &str,
    denom: &str,
    decimals: u32,
) -> Result<CosmosProvider<ModuleQueryClient>> {
    let conf = CosmosConnectionConf::new(
        vec![Url::parse(grpc_url).map_err(|e| eyre::eyre!("invalid gRPC URL: {}", e))?],
        vec![Url::parse(rpc_url).map_err(|e| eyre::eyre!("invalid RPC URL: {}", e))?],
        chain_id.to_string(),
        prefix.to_string(),
        denom.to_string(),
        RawCosmosAmount {
            amount: "100000000000.0".to_string(),
            denom: denom.to_string(),
        },
        32,
        OpSubmissionConfig::default(),
        NativeToken {
            decimals,
            denom: denom.to_string(),
        },
        1.0,
        None,
    )
    .map_err(|e| eyre::eyre!(e))?;

    let d = HyperlaneDomain::Known(KnownHyperlaneDomain::Osmosis);
    let locator = ContractLocator::new(&d, H256::zero());
    let signer = Some(key.signer());
    let metrics = PrometheusClientMetrics::default();
    let chain = None;

    CosmosProvider::<ModuleQueryClient>::new(&conf, &locator, signer, metrics, chain)
        .map_err(eyre::Report::from)
}

async fn wait_for_hub_balance_increase(
    provider: &CosmosProvider<ModuleQueryClient>,
    address: &str,
    denom: &str,
    initial_balance: U256,
    expected_increase: u64,
    timeout: Duration,
) -> bool {
    let start = std::time::Instant::now();
    let poll_interval = Duration::from_secs(3);
    let status_interval = Duration::from_secs(10);
    let mut last_status = std::time::Instant::now();
    let mut last_balance: Option<U256> = None;
    let target_balance = initial_balance + U256::from(expected_increase);

    loop {
        match provider.rpc().get_balance_denom(address.to_string(), denom.to_string()).await {
            Ok(balance) => {
                last_balance = Some(balance);
                if balance >= target_balance {
                    return true;
                }
            }
            Err(e) => {
                eprintln!("  Warning: Failed to query hub balance: {}", e);
                let _ = io::stderr().flush();
            }
        }

        if start.elapsed() >= timeout {
            return false;
        }

        // Print status every 10 seconds
        if last_status.elapsed() >= status_interval {
            let elapsed = start.elapsed().as_secs();
            let balance_str = last_balance.map(|b| b.to_string()).unwrap_or_else(|| "unknown".to_string());
            println!("  ... waiting {}s (current: {}, target: {})", elapsed, balance_str, target_balance);
            let _ = io::stdout().flush();
            last_status = std::time::Instant::now();
        }

        tokio::time::sleep(poll_interval).await;
    }
}

async fn wait_for_kaspa_balance_increase(
    client: &HttpClient,
    address: &str,
    initial_balance: i64,
    expected_increase: u64,
    timeout: Duration,
) -> bool {
    let start = std::time::Instant::now();
    let poll_interval = Duration::from_secs(3);
    let status_interval = Duration::from_secs(10);
    let mut last_status = std::time::Instant::now();
    let mut last_balance: Option<i64> = None;
    let target_balance = initial_balance + expected_increase as i64;

    loop {
        match client.get_balance_by_address(address).await {
            Ok(balance) => {
                last_balance = Some(balance);
                if balance >= target_balance {
                    return true;
                }
            }
            Err(e) => {
                eprintln!("  Warning: Failed to query kaspa balance: {}", e);
                let _ = io::stderr().flush();
            }
        }

        if start.elapsed() >= timeout {
            return false;
        }

        // Print status every 10 seconds
        if last_status.elapsed() >= status_interval {
            let elapsed = start.elapsed().as_secs();
            let balance_str = last_balance.map(|b| b.to_string()).unwrap_or_else(|| "unknown".to_string());
            println!("  ... waiting {}s (current: {}, target: {})", elapsed, balance_str, target_balance);
            let _ = io::stdout().flush();
            last_status = std::time::Instant::now();
        }

        tokio::time::sleep(poll_interval).await;
    }
}
