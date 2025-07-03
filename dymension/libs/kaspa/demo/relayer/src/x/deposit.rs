#![allow(unused)] // TODO: remove

use api_rs::apis::configuration;
use bytes::Bytes;
use corelib::api::deposits::Deposit;
use corelib::deposit::*;
use corelib::escrow::*;
use corelib::user::deposit::{deposit as do_deposit, deposit_impl};
use corelib::util::*;
use corelib::wallet::*;
use dymension_kaspa::KaspaHttpClient;
use dymension_kaspa::RestProvider;
use hardcode::e2e::*;
use hex;
use hyperlane_core::ChainCommunicationError;
use hyperlane_core::ChainResult;
use hyperlane_core::{Decode, Encode, HyperlaneMessage, H256, U256};
use hyperlane_metric::prometheus_metric::ChainInfo;
use hyperlane_metric::prometheus_metric::ClientConnectionType;
use hyperlane_metric::prometheus_metric::PrometheusClientMetrics;
use hyperlane_metric::prometheus_metric::PrometheusConfig;
use hyperlane_warp_route::TokenMessage;
use kaspa_addresses::Address;
use kaspa_consensus_core::{
    constants::TX_VERSION,
    sign::sign,
    subnets::SUBNETWORK_ID_NATIVE,
    tx::{
        MutableTransaction, ScriptPublicKey, Transaction, TransactionInput, TransactionOutpoint,
        TransactionOutput, UtxoEntry,
    },
};
use kaspa_core::info as KaspaInfo;
use kaspa_grpc_client::GrpcClient;
use kaspa_wallet_core::api::{AccountsSendRequest, WalletApi};
use kaspa_wallet_core::error::Error as KaspaError;
use kaspa_wallet_core::tx::Fees;
use kaspa_wallet_core::utxo::NetworkParams;
use relayer::deposit::handle_new_deposit;
use relayer::withdraw::*;
use reqwest::Url;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use validator::deposit::validate_deposit;
use validator::withdraw::*;

use kaspa_wallet_core::prelude::*;
use kaspa_wallet_pskt::prelude::*; // Import the prelude for easy access to traits/structs

use secp256k1::{rand::thread_rng, Keypair};
use tracing::{error, info, info_span, warn, Instrument};

use api_rs::apis::kaspa_transactions_api::{
    get_transaction_transactions_transaction_id_get,
    GetTransactionTransactionsTransactionIdGetParams,
};
use kaspa_rpc_core::api::rpc::RpcApi;
use workflow_core::abortable::Abortable;

use tokio::{sync::Mutex, task::JoinHandle, time};
use tokio_metrics::TaskMonitor;
pub struct DemoArgs {
    pub amt: u64,
    pub escrow_address: Address,
    pub payload: Option<String>,
    pub only_deposit: bool,
    pub wallet_secret: String,
}

impl Default for DemoArgs {
    fn default() -> Self {
        Self {
            amt: 1000000000000000000,
            escrow_address: Address::try_from(ESCROW_ADDRESS).unwrap(),
            payload: None,
            only_deposit: false,
            wallet_secret: "".to_string(),
        }
    }
}

/// dococo
pub async fn get_deposits(client: &KaspaHttpClient, address: String) -> ChainResult<Vec<Deposit>> {

    let res = client.client.get_deposits(&address).await;
    res.map_err(|e| ChainCommunicationError::from_other_str(&e.to_string()))
        .map(|deposits| {
            deposits
                .into_iter()
                //.filter(|d| d.payload.is_some())
                .collect()
        })
}

async fn deposit_loop(client: &KaspaHttpClient, address: String) {
    info!("Dymension, starting deposit detection loop");
    loop {
        let deposits_res: std::result::Result<Vec<Deposit>, ChainCommunicationError> = get_deposits(client,address.clone()).await;
        let deposits = match deposits_res {
            Ok(deposits) => deposits,
            Err(e) => {
                error!("Query new Kaspa deposits: {:?}", e);
                continue;
            }
        };
        //let mut deposits_new = Vec::new();
        for d in deposits.into_iter() {
            info!("Dymension, new deposit seen: {:?}", d);
        }
        time::sleep(Duration::from_secs(10)).await;

    }
}

pub async fn demo(args: DemoArgs) -> Result<(), Box<dyn Error>> {
    kaspa_core::log::init_logger(None, "");

    let s = Secret::from(args.wallet_secret);
    let w = get_wallet(&s, NETWORK_ID, URL.to_string()).await?;

    println!("address {}", &w.account()?.receive_address()?);
    println!("balance {}", &w.account()?.get_list_string()?);

    // deposit to escrow address
    let amt = args.amt;
    let escrow_address = args.escrow_address;

    let tx_id = if let Some(payload) = args.payload {
        info!("Dymension, sending deposit with payload: {:?}", payload);
        // deposit_impl(&w, &s, escrow_address.clone(), amt, payload.as_bytes().to_vec()).await?
        let bz = hex::decode(payload).unwrap();
        deposit_impl(&w, &s, escrow_address.clone(), amt, bz).await?
    } else {
        do_deposit(&w, &s, escrow_address.clone(), amt).await?
    };

    info!("Sent deposit transaction: {}", tx_id);

    if args.only_deposit {
        return Ok(());
    }

    //let chain = ChainInfo::new();
    //let config: configuration::Configuration = hardcode::e2e::get_tn10_config();
    let url = Url::parse("https://api-tn10.kaspa.org/").unwrap();
    let metrics_config = PrometheusConfig::from_url( &url, ClientConnectionType::Rpc, None);
    let metrics: PrometheusClientMetrics = PrometheusClientMetrics::default();
    let url = "https://api-tn10.kaspa.org/";
    let client: KaspaHttpClient = KaspaHttpClient::from_url(url.to_string(),metrics,metrics_config)?;

    let handle = tokio::spawn(async move {
                deposit_loop(&client,escrow_address.address_to_string()).await;
    });
    workflow_core::task::sleep(std::time::Duration::from_secs(120)).await;
/*
    // rpc config

    // api request
    let get_params = GetTransactionTransactionsTransactionIdGetParams {
        transaction_id: tx_id.to_string(),
        block_hash: None,
        inputs: None,
        outputs: None,
        resolve_previous_outpoints: None,
    };

    // get transaction info using Kaspa API
    let res = get_transaction_transactions_transaction_id_get(&config, get_params).await?;

    // build deposit from api response
    let d = Deposit::try_from(res)?;

    // handle deposit (relayer operation)
    let deposit_fxg = handle_new_deposit(&escrow_address.to_string(), &d).await?;

    // deposit encode to bytes
    let deposit_bytes_recv: Bytes = (&deposit_fxg).into();

    // deposit from bytes
    let deposit_recv = DepositFXG::try_from(deposit_bytes_recv)?;

    println!(
        "Deposit pulled by relay tx_id:{} block_id:{} amount:{}",
        deposit_recv.tx_id, deposit_recv.block_id, deposit_recv.amount
    );

    // validate deposit using kaspa rpc (validator operation)
    let validation_result = validate_deposit(
        &w.rpc_api(),
        &deposit_recv,
        &escrow_address.to_string(),
        NetworkParams::from(w.network_id()?),
    )
    .await?;

    if validation_result {
        println!("Deposit validated");
    } else {
        println!("Failed to validate deposit");
    }*/

    w.stop().await?;
    Ok(())
}
