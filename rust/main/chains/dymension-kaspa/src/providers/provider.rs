use std::ops::Deref;

use derive_new::new;
use eyre::Result as EyreResult;
use kaspa_wallet_pskt::prelude::*;
use tonic::async_trait;

use dym_kas_core::withdraw::WithdrawFXG;
use hyperlane_core::{
    rpc_clients::FallbackProvider, BlockInfo, ChainInfo, ChainResult, ContractLocator,
    HyperlaneChain, HyperlaneDomain, HyperlaneProvider, HyperlaneProviderError, TxnInfo, H256,
    H512, U256,
};
use hyperlane_metric::prometheus_metric::PrometheusClientMetrics;
use kaspa_wallet_pskt::prelude::Bundle;

use super::validators::ValidatorsClient;
use super::RestProvider;

use crate::ConnectionConf;
use eyre::Result;

use hyperlane_cosmos_native::Signer;

/// dococo
#[derive(Debug, Clone)]
pub struct KaspaProvider {
    domain: HyperlaneDomain,
    conf: ConnectionConf,
    rest: RestProvider,
    // TODO: wrpc
    validators: ValidatorsClient,
}

impl KaspaProvider {
    /// dococo
    pub fn new(
        conf: &ConnectionConf,
        locator: &ContractLocator,
        signer: Option<Signer>,
        metrics: PrometheusClientMetrics,
        chain: Option<hyperlane_metric::prometheus_metric::ChainInfo>,
    ) -> ChainResult<Self> {
        let rest = RestProvider::new(conf.clone(), signer, metrics.clone(), chain.clone())?;
        let validators = ValidatorsClient::new(conf.clone())?;

        Ok(KaspaProvider {
            domain: locator.domain.clone(),
            conf: conf.clone(),
            rest,
            validators,
        })
    }

    /// dococo
    pub fn rest(&self) -> &RestProvider {
        &self.rest
    }

    /// dococo
    pub fn validators(&self) -> &ValidatorsClient {
        &self.validators
    }

    /// dococo
    pub async fn process_withdrawal(&self, fxg: &WithdrawFXG) -> Result<()> {
        let bundles = self.validators().get_withdraw_sigs(fxg).await?;
        let txs_sigs = combine_validator_bundles(bundles)?;
        Ok(())
    }
}

impl HyperlaneChain for KaspaProvider {
    /// Return the domain
    fn domain(&self) -> &HyperlaneDomain {
        &self.domain
    }

    /// A provider for the chain
    fn provider(&self) -> Box<dyn HyperlaneProvider> {
        Box::new(self.clone())
    }
}

#[async_trait]
impl HyperlaneProvider for KaspaProvider {
    // only used by scraper
    async fn get_block_by_height(&self, height: u64) -> ChainResult<BlockInfo> {
        Err(HyperlaneProviderError::CouldNotFindBlockByHeight(height).into())
    }

    // only used by scraper
    async fn get_txn_by_hash(&self, hash: &H512) -> ChainResult<TxnInfo> {
        return Err(HyperlaneProviderError::CouldNotFindTransactionByHash(*hash).into());
    }

    async fn is_contract(&self, _address: &H256) -> ChainResult<bool> {
        // TODO: check if the address is a recipient (this is a hyperlane team todo)
        return Ok(true);
    }

    async fn get_balance(&self, address: String) -> ChainResult<U256> {
        // TODO: maybe I can return just a larger number here?
        return Ok(0.into());
    }

    async fn get_chain_metrics(&self) -> ChainResult<Option<ChainInfo>> {
        return Ok(None);
    }
}

fn combine_validator_bundles(bundles: Vec<Bundle>) -> EyreResult<Vec<PSKT<Combiner>>> {
    // each bundle is from a different validator, and is a vector of pskt
    // therefore index i of each vector corresponds to the same TX i

    let validators = bundles
        .iter()
        .map(|b| {
            b.iter()
                .map(|inner| PSKT::<Signer>::from(inner.clone()))
                .collect::<Vec<PSKT<Signer>>>()
        })
        .collect::<Vec<Vec<PSKT<Signer>>>>();

    let n_txs = validators.first().unwrap().len();

    // need to walk across each tx, and for each tx walk across each signer, and combine all for that tx
    let mut tx_sigs: Vec<Vec<PSKT<Signer>>> = Vec::new();
    for tx_i in 0..n_txs {
        let mut sigs_for_tx = Vec::new();
        for val_tx_sigs in validators.iter() {
            sigs_for_tx.push(val_tx_sigs[tx_i].clone());
        }
        tx_sigs.push(sigs_for_tx);
    }

    let mut ret = Vec::new();
    for val_sig in tx_sigs.iter() {
        let mut combiner = val_sig.first().unwrap().clone().combiner();
        for tx_sig in val_sig.iter().skip(1) {
            combiner = (combiner + tx_sig.clone()).unwrap();
        }
        ret.push(combiner);
    }
    Ok(ret)
}
