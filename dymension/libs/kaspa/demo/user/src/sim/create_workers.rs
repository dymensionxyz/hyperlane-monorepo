use super::worker::Worker;
use corelib::wallet::{EasyKaspaWallet, EasyKaspaWalletArgs, Network};
use cosmos_sdk_proto::cosmos::bank::v1beta1::MsgSend;
use cosmos_sdk_proto::cosmos::base::v1beta1::Coin;
use cosmos_sdk_proto::traits::Message;
use cosmrs::Any;
use eyre::Result;
use hyperlane_core::{
    config::OpSubmissionConfig, ContractLocator, HyperlaneDomain, KnownHyperlaneDomain,
    NativeToken, H256,
};
use hyperlane_cosmos::RawCosmosAmount;
use hyperlane_cosmos::{
    native::ModuleQueryClient, ConnectionConf as CosmosConnectionConf, CosmosProvider,
};
use kaspa_wallet_core::prelude::Secret;
use std::time::Duration;
use tracing::{debug, info};
use url::Url;

use crate::x::args::CreateWorkersCli;

pub struct CreateWorkersArgs {
    pub num_workers: usize,
    pub workers_dir: String,
    pub kaspa_fund_amount: u64,
    pub hub_fund_amount: u64,
    pub wrpc_url: String,
    pub wallet_secret: String,
    pub wallet_dir: Option<String>,
    pub hub_whale_priv_key: String,
    pub hub_rpc_url: String,
    pub hub_grpc_url: String,
    pub hub_chain_id: String,
    pub hub_prefix: String,
    pub hub_denom: String,
    pub hub_decimals: u32,
}

impl TryFrom<CreateWorkersCli> for CreateWorkersArgs {
    type Error = eyre::Error;

    fn try_from(cli: CreateWorkersCli) -> Result<Self, Self::Error> {
        Ok(CreateWorkersArgs {
            num_workers: cli.num_workers,
            workers_dir: cli.workers_dir,
            kaspa_fund_amount: cli.kaspa_fund_amount,
            hub_fund_amount: cli.hub_fund_amount,
            wrpc_url: cli.wallet.rpc_url,
            wallet_secret: cli.wallet.wallet_secret,
            wallet_dir: cli.wallet.wallet_dir,
            hub_whale_priv_key: cli.hub_whale_priv_key,
            hub_rpc_url: cli.hub_rpc_url,
            hub_grpc_url: cli.hub_grpc_url,
            hub_chain_id: cli.hub_chain_id,
            hub_prefix: cli.hub_prefix,
            hub_denom: cli.hub_denom,
            hub_decimals: cli.hub_decimals,
        })
    }
}

async fn create_hub_provider(
    signer_key_hex: &str,
    rpc_url: &str,
    grpc_url: &str,
    chain_id: &str,
    prefix: &str,
    denom: &str,
    decimals: u32,
) -> Result<CosmosProvider<ModuleQueryClient>> {
    let conf = CosmosConnectionConf::new(
        vec![Url::parse(grpc_url).map_err(|e| eyre::eyre!("Invalid gRPC URL: {}", e))?],
        vec![Url::parse(rpc_url).map_err(|e| eyre::eyre!("Invalid RPC URL: {}", e))?],
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
    let hub_key = super::key_cosmos::EasyHubKey::from_hex(signer_key_hex);
    let signer = Some(hub_key.signer());
    debug!("hub whale signer: {:?}", signer);
    let metrics = hyperlane_metric::prometheus_metric::PrometheusClientMetrics::default();
    let chain = None;
    CosmosProvider::<ModuleQueryClient>::new(&conf, &locator, signer, metrics, chain)
        .map_err(eyre::Report::from)
}

async fn fund_hub_address(
    hub: &CosmosProvider<ModuleQueryClient>,
    to_address: String,
    amount: u64,
    denom: &str,
) -> Result<()> {
    debug!("funding hub address: {}", to_address);
    let rpc = hub.rpc();
    let msg = MsgSend {
        from_address: rpc.get_signer()?.address_string.clone(),
        to_address: to_address.clone(),
        amount: vec![Coin {
            amount: amount.to_string(),
            denom: denom.to_string(),
        }],
    };
    let a = Any {
        type_url: "/cosmos.bank.v1beta1.MsgSend".to_string(),
        value: msg.encode_to_vec(),
    };
    let gas_limit = None;
    let response = rpc.send(vec![a], gas_limit).await;
    match response {
        Ok(response) => {
            if response.check_tx.code.is_err() {
                return Err(eyre::eyre!(
                    "Transaction failed during CheckTx with code {:?}: {}",
                    response.check_tx.code,
                    response.check_tx.log
                ));
            }
            if response.tx_result.code.is_err() {
                return Err(eyre::eyre!(
                    "Transaction failed during DeliverTx with code {:?}: {}",
                    response.tx_result.code,
                    response.tx_result.log
                ));
            }
            debug!("Funded hub address: {}", to_address);
            Ok(())
        }
        Err(e) => Err(eyre::eyre!("Failed to fund hub address: {:?}", e)),
    }
}

pub async fn create_and_fund_workers(args: CreateWorkersArgs) -> Result<()> {
    let net = Network::KaspaTest10;
    let whale_secret = Secret::from(args.wallet_secret.clone());

    let kaspa_whale = EasyKaspaWallet::try_new(EasyKaspaWalletArgs {
        wallet_secret: args.wallet_secret,
        wrpc_url: args.wrpc_url.clone(),
        net: net.clone(),
        storage_folder: args.wallet_dir,
        new: false,
    })
    .await?;

    let hub_whale = create_hub_provider(
        &args.hub_whale_priv_key,
        &args.hub_rpc_url,
        &args.hub_grpc_url,
        &args.hub_chain_id,
        &args.hub_prefix,
        &args.hub_denom,
        args.hub_decimals,
    )
    .await?;

    std::fs::create_dir_all(&args.workers_dir)?;

    info!(
        "Creating and funding {} workers (Kaspa + Hub) in {}",
        args.num_workers, args.workers_dir
    );

    for i in 0..args.num_workers {
        let worker =
            Worker::create_new(i, args.wrpc_url.clone(), net.clone(), &args.workers_dir).await?;

        let kaspa_address = worker.change_address()?;
        let hub_address = worker.hub_key.signer().address_string.clone();

        use kaspa_wallet_core::tx::{Fees, PaymentDestination, PaymentOutput};

        let dst = PaymentDestination::from(PaymentOutput::new(
            kaspa_address.clone(),
            args.kaspa_fund_amount,
        ));
        let fees = Fees::from(0i64);

        kaspa_whale
            .wallet
            .account()?
            .send(
                dst,
                None,
                fees,
                None,
                whale_secret.clone(),
                None,
                &workflow_core::abortable::Abortable::new(),
                None,
            )
            .await?;

        fund_hub_address(
            &hub_whale,
            hub_address.clone(),
            args.hub_fund_amount,
            &args.hub_denom,
        )
        .await?;

        info!(
            "Created and funded worker {}/{}: kaspa={}, hub={}",
            i + 1,
            args.num_workers,
            kaspa_address,
            hub_address
        );

        tokio::time::sleep(Duration::from_millis(2000)).await;
    }

    info!(
        "Successfully created and funded {} workers in {}",
        args.num_workers, args.workers_dir
    );

    Ok(())
}
