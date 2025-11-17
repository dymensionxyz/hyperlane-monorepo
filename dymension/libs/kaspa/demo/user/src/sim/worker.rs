use super::key_cosmos::EasyHubKey;
use corelib::wallet::{EasyKaspaWallet, EasyKaspaWalletArgs, Network};
use eyre::Result;
use kaspa_addresses::Address;
use kaspa_consensus_core::tx::TransactionId;
use std::path::PathBuf;

#[derive(Clone)]
pub struct Worker {
    pub wallet: EasyKaspaWallet,
    pub hub_key: EasyHubKey,
    pub worker_id: usize,
}

const SECRET: &str = "lkjsdf";
const HUB_KEY_FILENAME: &str = "hub_key.hex";

impl Worker {
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

        let hub_key = EasyHubKey::new();

        let hub_key_path = worker_storage.join(HUB_KEY_FILENAME);
        let hub_key_hex = hex::encode(hub_key.private_key_bytes());
        std::fs::write(hub_key_path, hub_key_hex)?;

        Ok(Self {
            wallet,
            hub_key,
            worker_id,
        })
    }

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

        let hub_key_path = worker_storage.join(HUB_KEY_FILENAME);
        let hub_key_hex = std::fs::read_to_string(hub_key_path)?;
        let hub_key = EasyHubKey::from_hex(&hub_key_hex);

        Ok(Self {
            wallet,
            hub_key,
            worker_id,
        })
    }

    pub fn receive_address(&self) -> Result<Address> {
        Ok(self.wallet.account().receive_address()?)
    }

    pub fn change_address(&self) -> Result<Address> {
        Ok(self.wallet.account().change_address()?)
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
