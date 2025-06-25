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
    KnownHyperlaneDomain, TxnInfo, H256, H512, U256,
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
    network_info: NetworkInfo,
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
        let info = NetworkInfo::new(args.domain, args.rpc_url);
        let w = get_wallet(&s, info.network_id, info.rpc_url)
            .await
            .wrap_err("Failed to get wallet")?;
        Ok(Self {
            wallet: w,
            network_info: NetworkInfo::new(args.domain, args.rpc_url),
        })
    }

    pub fn network(&self) -> NetworkType {
        self.network_info.network_type
    }

    pub fn network_id(&self) -> NetworkId {
        self.network_info.network_id
    }

    pub fn address_prefix(&self) -> Prefix {
        self.network_info.address_prefix
    }

    pub fn address_version(&self) -> Version {
        self.network_info.address_version
    }

    pub fn rpc_url(&self) -> Arc<DynRpcApi> {
        self.wallet.rpc_api()
    }

    pub fn api(&self) -> Arc<DynRpcApi> {
        self.wallet.rpc_api()
    }

    pub fn account(&self) -> Arc<dyn Account> {
        self.wallet.account()?
    }

}

struct NetworkInfo {
    pub network_id: NetworkId,
    pub network_type: NetworkType,
    pub address_prefix: Prefix,
    pub address_version: Version,
    pub rpc_url: String,
    pub domain: HyperlaneDomain,
}

impl NetworkInfo {
    pub fn new(domain: HyperlaneDomain, rpc_url: String) -> Self {
        match domain {
            HyperlaneDomain::Known(KnownHyperlaneDomain::KaspaTest10) => Self {
                network_id: NetworkId::with_suffix(NetworkType::Testnet, 10),
                network_type: NetworkType::Testnet,
                address_prefix: Prefix::Testnet,
                address_version: Version::PubKey,
                rpc_url,
                domain,
            },
            _ => todo!("only tn10 supported"),
        }
        // TODO: finish
    }
}
