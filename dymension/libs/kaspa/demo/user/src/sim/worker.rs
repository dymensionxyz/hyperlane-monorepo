use corelib::wallet::{EasyKaspaWallet, EasyKaspaWalletArgs, Network};
use eyre::Result;
use kaspa_addresses::Address;
use kaspa_consensus_core::tx::TransactionId;
use std::path::PathBuf;

/// Worker wallet for parallel deposits
/// Each worker uses an independent wallet
#[derive(Clone)]
pub struct WorkerWallet {
    pub wallet: EasyKaspaWallet,
    pub worker_id: usize,
}

const SECRET: &str = "lkjsdf";

impl WorkerWallet {
    /// Create a new worker wallet with its own storage in a permanent directory
    pub async fn create_new(
        worker_id: usize,
        wrpc_url: String,
        net: Network,
        workers_dir: &str,
    ) -> Result<Self> {
        let worker_storage = PathBuf::from(workers_dir).join(format!("worker-{}", worker_id));
        std::fs::create_dir_all(&worker_storage)?;

        let wallet = EasyKaspaWallet::try_new(EasyKaspaWalletArgs {
            wallet_secret: SECRET.to_string(),
            wrpc_url,
            net,
            storage_folder: Some(worker_storage.to_string_lossy().to_string()),
            new: true,
        })
        .await?;

        Ok(Self { wallet, worker_id })
    }

    /// Load an existing worker wallet from a permanent directory
    pub async fn load_existing(
        worker_id: usize,
        wrpc_url: String,
        net: Network,
        workers_dir: &str,
    ) -> Result<Self> {
        let worker_storage = PathBuf::from(workers_dir).join(format!("worker-{}", worker_id));

        let wallet = EasyKaspaWallet::try_new(EasyKaspaWalletArgs {
            wallet_secret: SECRET.to_string(),
            wrpc_url,
            net,
            storage_folder: Some(worker_storage.to_string_lossy().to_string()),
            new: false,
        })
        .await?;

        Ok(Self { wallet, worker_id })
    }

    pub fn receive_address(&self) -> Result<Address> {
        Ok(self.wallet.account().receive_address()?)
    }

    pub async fn deposit_with_payload(
        &self,
        address: Address,
        amt: u64,
        payload: Vec<u8>,
    ) -> Result<TransactionId> {
        corelib::user::deposit::deposit_with_payload(
            &self.wallet.wallet,
            &self.wallet.secret,
            address,
            amt,
            payload,
        )
        .await
        .map_err(Into::into)
    }
}
