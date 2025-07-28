use super::key_cosmos::EasyHubKey;
use super::round_trip::do_round_trip;
use super::round_trip::TaskArgs;
use super::round_trip::TaskResources;
use super::stats::render_stats;
use super::util::som_to_kas;
use corelib::api::base::RateLimitConfig;
use corelib::api::client::HttpClient;
use corelib::wallet::EasyKaspaWallet;
use corelib::wallet::{EasyKaspaWalletArgs, Network};
use eyre::Result;
use hardcode;
use hyperlane_cosmos_native::ConnectionConf as CosmosConnectionConf;
use hyperlane_cosmos_native::CosmosNativeProvider;
use rand_distr::{Distribution, Exp};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::x::args::{SimulateTrafficCli, WalletCli};
use cosmos_sdk_proto::cosmos::bank::v1beta1::MsgSend;
use cosmos_sdk_proto::cosmos::base::v1beta1::Coin;
use cosmos_sdk_proto::traits::Message;
use cosmrs::Any;
use hyperlane_core::config::OpSubmissionConfig;
use hyperlane_core::ContractLocator;
use hyperlane_core::HyperlaneDomain;
use hyperlane_core::KnownHyperlaneDomain;
use hyperlane_core::NativeToken;
use hyperlane_core::H256;
use hyperlane_cosmos_native::RawCosmosAmount;
use hyperlane_metric::prometheus_metric::PrometheusClientMetrics;
use tracing::info;
use url::Url;

async fn cosmos_provider() -> Result<CosmosNativeProvider> {
    let conf = CosmosConnectionConf::new(
        vec![Url::parse("https://rpc-dymension-playground35.mzonder.com:443").unwrap()],
        vec![Url::parse("https://grpc-dymension-playground35.mzonder.com:443").unwrap()],
        "dymension_3405-1".to_string(),
        "dym".to_string(),
        "adym".to_string(),
        RawCosmosAmount {
            amount: "100000000000.0".to_string(),
            denom: "adym".to_string(),
        },
        1.0,
        32,
        OpSubmissionConfig::default(),
        NativeToken {
            decimals: 18,
            denom: "adym".to_string(),
        },
    );
    let d = HyperlaneDomain::Known(KnownHyperlaneDomain::Osmosis);
    let locator = ContractLocator::new(&d, H256::zero());
    let signer = None;
    let metrics = PrometheusClientMetrics::default();
    let chain = None;
    CosmosNativeProvider::new(&conf, &locator, signer, metrics, chain).map_err(eyre::Report::from)
}

pub struct Params {
    pub time_limit: Duration, // total target simulation time
    pub budget: u64,          // in sompi
    pub ops_per_minute: u64,  // osmosis does 90 per minute
    pub max_ops: u64,         // max number of ops to run, disregarding distributions
    pub min_value: u64,       // in sompi
    pub hub_fund_amount: u64, // in adym
}

impl Params {
    /// Used to draw value of each op, in sompi
    pub fn distr_value(&self) -> Exp<f64> {
        // TODO: need to use some clamping/minimum
        Exp::new(1.0 / self.op_budget()).unwrap()
    }
    pub fn sample_value(&self) -> u64 {
        // TODO: use proper clamping, or this will blow the budget
        let v = self.distr_value().sample(&mut rand::rng()) as u64;
        if v < self.min_value {
            self.min_value
        } else {
            v
        }
    }
    /// Used to draw time between ops, in milliseconds
    pub fn distr_time(&self) -> Exp<f64> {
        Exp::new(self.ops_per_second() / 1000.0).unwrap()
    }
    pub fn num_ops(&self) -> f64 {
        self.time_limit.as_secs_f64() * self.ops_per_second()
    }
    pub fn op_budget(&self) -> f64 {
        self.budget as f64 / self.num_ops()
    }
    pub fn ops_per_second(&self) -> f64 {
        self.ops_per_minute as f64 / 60.0
    }
}

pub struct SimulateTrafficArgs {
    pub params: Params,
    pub task_args: TaskArgs,
    pub wallet: WalletCli,
}

impl TryFrom<SimulateTrafficCli> for SimulateTrafficArgs {
    type Error = eyre::Error;

    fn try_from(cli: SimulateTrafficCli) -> Result<Self, Self::Error> {
        let addr = kaspa_addresses::Address::try_from(cli.escrow_address.clone())?;
        Ok(SimulateTrafficArgs {
            params: Params {
                time_limit: std::time::Duration::from_secs(cli.time_limit),
                budget: cli.budget,
                ops_per_minute: cli.ops_per_minute,
                max_ops: cli.max_ops,
                min_value: hardcode::tx::MIN_DEPOSIT_AMOUNT,
                hub_fund_amount: 200000000000000, // should be enough to pay fees
            },
            task_args: TaskArgs {
                domain_kas: cli.domain_kas,
                token_kas_placeholder: cli.token_kas_placeholder,
                domain_hub: cli.domain_hub,
                token_hub: cli.token_hub,
                escrow_address: addr,
                hl_token_denom: cli.hl_token_denom,
            },
            wallet: cli.wallet,
        })
    }
}

pub struct TrafficSim {
    params: Params,
    resources: TaskResources,
}

impl TrafficSim {
    pub async fn new(args: SimulateTrafficArgs) -> Result<Self> {
        let w = EasyKaspaWallet::try_new(EasyKaspaWalletArgs {
            wallet_secret: args.wallet.wallet_secret,
            rpc_url: "localhost:17210".to_string(),
            net: Network::KaspaTest10,
            storage_folder: None,
        })
        .await?;
        let resources = TaskResources {
            w: w.clone(),
            args: args.task_args,
            hub: cosmos_provider().await?,
            kas_rest: HttpClient::new(
                "https://api-tn10.kaspa.org/".to_string(),
                RateLimitConfig::default(),
            ),
        };
        Ok(TrafficSim {
            params: args.params,
            resources,
        })
    }

    pub async fn run(&self) -> Result<()> {
        let mut rng = rand::rng();
        let start_time = Instant::now();
        let mut total_ops = 0;
        let mut total_spend = 0;

        let (stats_tx, mut stats_rx) = mpsc::channel(100);

        let collector_handle = tokio::spawn(async move {
            let mut collected_stats = Vec::new();
            while let Some(stats) = stats_rx.recv().await {
                collected_stats.push(stats);
            }
            collected_stats
        });

        info!("Starting tasks");
        while start_time.elapsed() < self.params.time_limit {
            let nominal_value = self.params.sample_value();
            let tx_clone = stats_tx.clone();
            let r = self.resources.clone();
            let task_id = total_ops;
            let hub_key = EasyHubKey::new();
            fund_hub_addr(&hub_key, &r.hub).await?;
            tokio::spawn(async move {
                do_round_trip(r, nominal_value, &tx_clone, task_id, hub_key).await;
                drop(tx_clone); // TODO: needed?
            });
            total_spend += nominal_value;
            total_ops += 1;
            let sleep_millis = self.params.distr_time().sample(&mut rng) as u64;
            if self.params.max_ops > 0 && total_ops >= self.params.max_ops {
                break;
            }
            info!("Sleeping for {} ms", sleep_millis);
            tokio::time::sleep(Duration::from_millis(sleep_millis)).await;
            info!(
                "elasped millis {}, interval {}, value {}",
                start_time.elapsed().as_millis(),
                sleep_millis,
                som_to_kas(nominal_value)
            );
        }
        info!("Waiting for tasks to finish");

        drop(stats_tx); // TODO: need to do this on each sender?
        let final_stats = collector_handle.await?;
        render_stats(final_stats, total_spend, total_ops);

        Ok(())
    }
}

async fn fund_hub_addr(
    hub_key: &EasyHubKey,
    hub: &CosmosNativeProvider,
    amount: u64,
) -> Result<()> {
    let hub_addr = hub_key.signer().address_string.clone();
    let rpc = hub.rpc();
    let msg = MsgSend {
        from_address: rpc.get_signer()?.address_string.clone(),
        to_address: hub_addr.clone(),
        amount: vec![Coin {
            amount: amount.to_string(),
            denom: "adym".to_string(),
        }],
    };
    let a = Any {
        type_url: "/cosmos.bank.v1beta1.MsgSend".to_string(),
        value: msg.encode_to_vec(),
    };
    let gas_limit = None;
    let response = rpc.send(vec![a], gas_limit).await?;
    if response.tx_result.code.is_ok() {
        Ok(())
    } else {
        Err(eyre::eyre!("Failed to fund hub address"))
    }
}
