use std::ops::Deref;

use kaspa_addresses::{Prefix, Version};
use kaspa_consensus_core::network::{NetworkId, NetworkType};
use kaspa_wallet_core::wallet::Wallet;

use derive_new::new;
use eyre::Result as EyreResult;
use kaspa_wallet_pskt::prelude::*;
use tonic::async_trait;

use dym_kas_core::withdraw::WithdrawFXG;
use dym_kas_relayer::withdraw_construction::on_new_withdrawals;
use hyperlane_core::{
    rpc_clients::FallbackProvider, BlockInfo, ChainInfo, ChainResult, ContractLocator,
    HyperlaneChain, HyperlaneDomain, HyperlaneMessage, HyperlaneProvider, HyperlaneProviderError,
    TxnInfo, H256, H512, U256,
};
use hyperlane_metric::prometheus_metric::PrometheusClientMetrics;
use kaspa_consensus_core::tx::Transaction;
use kaspa_wallet_pskt::prelude::Bundle;

use super::validators::ValidatorsClient;
use super::RestProvider;

use crate::ConnectionConf;
use eyre::Result;

use hyperlane_cosmos_native::Signer as HyperlaneSigner;
struct EasyKaspaWallet {
    // wallet: Wallet,
    // domain: HyperlaneDomain,
}

struct EasyKaspaWalletArgs {
    priv_key: String,
    wallet_secret: String,
    // network_id: NetworkId,
    rpc_url: String, // .e.g localhost:16210
}

impl EasyKaspaWallet {
    pub async fn new() -> Self {
        Self {}
    }

    pub fn network(&self) -> NetworkType {
        todo!()
    }

   pub fn network_id(&self) -> NetworkId {
    todo!()
   }

   pub fn address_prefix(&self) -> Prefix {
    todo!()

   }

   pub fn address_version(&self) -> Version {
    return Version::PubKey;
   }

    
    
}