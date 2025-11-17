use super::worker::WorkerWallet;
use corelib::wallet::{EasyKaspaWallet, EasyKaspaWalletArgs, Network};
use eyre::Result;
use kaspa_wallet_core::prelude::Secret;
use std::time::Duration;
use tracing::info;

use crate::x::args::CreateWorkersCli;

pub struct CreateWorkersArgs {
    pub num_workers: usize,
    pub workers_dir: String,
    pub fund_amount: u64,
    pub wrpc_url: String,
    pub wallet_secret: String,
    pub wallet_dir: Option<String>,
}

impl TryFrom<CreateWorkersCli> for CreateWorkersArgs {
    type Error = eyre::Error;

    fn try_from(cli: CreateWorkersCli) -> Result<Self, Self::Error> {
        Ok(CreateWorkersArgs {
            num_workers: cli.num_workers,
            workers_dir: cli.workers_dir,
            fund_amount: cli.fund_amount,
            wrpc_url: cli.wallet.rpc_url,
            wallet_secret: cli.wallet.wallet_secret,
            wallet_dir: cli.wallet.wallet_dir,
        })
    }
}

pub async fn create_and_fund_workers(args: CreateWorkersArgs) -> Result<()> {
    let net = Network::KaspaTest10;
    let whale_secret = Secret::from(args.wallet_secret.clone());

    let whale_wallet = EasyKaspaWallet::try_new(EasyKaspaWalletArgs {
        wallet_secret: args.wallet_secret,
        wrpc_url: args.wrpc_url.clone(),
        net: net.clone(),
        storage_folder: args.wallet_dir,
        new: false,
    })
    .await?;

    std::fs::create_dir_all(&args.workers_dir)?;

    info!(
        "Creating and funding {} worker wallets in {}",
        args.num_workers, args.workers_dir
    );

    for i in 0..args.num_workers {
        let worker =
            WorkerWallet::create_new(i, args.wrpc_url.clone(), net.clone(), &args.workers_dir)
                .await?;

        let worker_address = worker.receive_address()?;

        use kaspa_wallet_core::tx::{Fees, PaymentDestination, PaymentOutput};

        let dst =
            PaymentDestination::from(PaymentOutput::new(worker_address.clone(), args.fund_amount));
        let fees = Fees::from(0i64);

        whale_wallet
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

        info!(
            "Created and funded worker {}/{} at {}",
            i + 1,
            args.num_workers,
            worker_address
        );

        tokio::time::sleep(Duration::from_millis(2000)).await;
    }

    info!(
        "Successfully created and funded {} workers in {}",
        args.num_workers, args.workers_dir
    );

    Ok(())
}
