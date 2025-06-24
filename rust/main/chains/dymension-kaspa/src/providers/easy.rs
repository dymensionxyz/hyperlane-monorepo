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
    HyperlaneChain, HyperlaneDomain, HyperlaneMessage, HyperlaneProvider, HyperlaneProviderError, KnownHyperlaneDomain,
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

use core::deposit::*;
use core::escrow::*;
use core::util::*;
use core::wallet::*;
use relayer::withdraw::*;
use validator::withdraw::*;
use x::args::Args;
use x::consts::*;

use std::sync::Arc;

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
use kaspa_core::info;
use kaspa_grpc_client::GrpcClient;
use kaspa_wallet_core::api::{AccountsSendRequest, WalletApi};
use kaspa_wallet_core::error::Error;
use kaspa_wallet_core::tx::Fees;

use kaspa_wallet_core::prelude::*;
use kaspa_wallet_pskt::prelude::*; // Import the prelude for easy access to traits/structs

use kaspa_txscript::{
    extract_script_pub_key_address, multisig_redeem_script, pay_to_address_script,
    pay_to_script_hash_script,
};

use secp256k1::{rand::thread_rng, Keypair};

use kaspa_rpc_core::api::rpc::RpcApi;
use workflow_core::abortable::Abortable;
struct EasyKaspaWallet {
    wallet: Wallet,
    domain: HyperlaneDomain,
}

struct EasyKaspaWalletArgs {
    priv_key: String,
    wallet_secret: String,
    // network_id: NetworkId,
    rpc_url: String, // .e.g localhost:16210
    domain: HyperlaneDomain,
}

impl EasyKaspaWallet {
    pub async fn try_new(args: EasyKaspaWalletArgs) -> Result<Self> {
        let s = Secret::from(args.wallet_secret);
        let w = get_wallet(&s, NETWORK_ID, args.rpc_url)
            .await
            .wrap_err("Failed to get wallet")?;
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

    pub fn rpc_url(&self) -> Arc<DynRpcApi> {
        self.wallet.rpc_api()
    }
}

struct NetworkInfo {
    network_id: NetworkId,
    network_type: NetworkType,
    address_prefix: Prefix,
    address_version: Version,
    rpc_url: String,
    domain: HyperlaneDomain,
}

impl NetworkInfo {
    pub fn new(domain: HyperlaneDomain) -> Self {
        match domain {
            KnownHyperlaneDomain::Kaspa => {
                Self {
                    network_id: NetworkId::Kaspa,
                    network_type: NetworkType::Kaspa,
                    address_prefix: Prefix::Kaspa,
                    address_version: Version::PubKey,
                    rpc_url: "localhost:16210".to_string(),
                }
        }
        // TODO: finish
    }
}
