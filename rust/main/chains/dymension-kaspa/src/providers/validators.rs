use tonic::async_trait;

use hyperlane_core::{
    rpc_clients::BlockNumberGetter, ChainCommunicationError, ChainResult, Checkpoint,
    CheckpointWithMessageId, Signature, SignedCheckpointWithMessageId, SignedType, H256, U256,
};

use bytes::Bytes;
use eyre::{eyre, Result};
use futures::stream::{self, StreamExt};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use tracing::{error, info};

use crate::ConnectionConf;

use crate::endpoints::*;
use axum::Json;
use dym_kas_core::{confirmation::ConfirmationFXG, deposit::DepositFXG, withdraw::WithdrawFXG};
use kaspa_wallet_pskt::prelude::Bundle;

#[derive(Debug, Clone)]
pub struct ValidatorsClient {
    pub conf: ConnectionConf,
}

#[async_trait]
impl BlockNumberGetter for ValidatorsClient {
    async fn get_block_number(&self) -> Result<u64, ChainCommunicationError> {
        return ChainResult::Err(ChainCommunicationError::from_other_str("not implemented"));
    }
}

impl ValidatorsClient {
    pub fn new(conf: ConnectionConf) -> ChainResult<Self> {
        Ok(ValidatorsClient { conf })
    }

    pub fn multisig_threshold_hub_ism(&self) -> usize {
        self.conf.multisig_threshold_hub_ism
    }
}

pub async fn get_deposit_sigs(
    &conf: &ConnectionConf,
    fxg: &DepositFXG,
) -> ChainResult<Vec<SignedCheckpointWithMessageId>> {
    get_signatures_from_validators(conf, fxg, ROUTE_VALIDATE_NEW_DEPOSITS, "deposit").await
}

pub async fn get_confirmation_sigs(
    &conf: &ConnectionConf,
    fxg: &ConfirmationFXG,
) -> ChainResult<Vec<Signature>> {
    get_signatures_from_validators(
        conf,
        fxg,
        ROUTE_VALIDATE_CONFIRMED_WITHDRAWALS,
        "confirmation",
    )
    .await
}

pub async fn get_withdraw_sigs(
    conf: &ConnectionConf,
    fxg: &WithdrawFXG,
) -> ChainResult<Vec<Bundle>> {
    get_signatures_from_validators(conf, fxg, ROUTE_SIGN_PSKTS, "withdrawal").await
}

pub fn multisig_threshold_hub_ism(&self) -> usize {
    self.conf.multisig_threshold_hub_ism
}

// --- Start of The Fix: Logic moved to a free-standing function ---

/// Generic function to request signatures from all validators in parallel.
/// By making this a free function, we avoid potential `Sync` issues on the `ValidatorsClient` struct.
async fn get_signatures_from_validators<'a, P, R>(
    conf: &ConnectionConf, // Takes config directly instead of `&self`
    payload: &'a P,
    endpoint: &str,
    context: &str,
) -> ChainResult<Vec<R>>
where
    &'a P: TryInto<Bytes> + Copy + Send + Sync,
    <&'a P as TryInto<Bytes>>::Error: Into<eyre::Report> + Send + Sync + 'static,
    R: DeserializeOwned + Send,
    P: Sync,
{
    info!(
        "Dymension, asking validators for {} sigs, number of validators: {}",
        context,
        conf.validator_hosts.len()
    );

    let results = stream::iter(&conf.validator_hosts)
        .map(|host| async move {
            let h = host.to_string();
            match request_validator_signature(&h, endpoint, payload, context).await {
                Ok(Some(sig)) => {
                    info!(
                        "Dymension, got {} sig response ok, validator: {:?}",
                        context, h
                    );
                    Some(sig)
                }
                Ok(None) => {
                    error!(
                        "Dymension, got {} sig response None, validator: {:?}",
                        context, h
                    );
                    None
                }
                Err(e) => {
                    error!(
                        "Dymension, got {} sig response Err, validator: {:?}, error: {:?}",
                        context, h, e
                    );
                    None
                }
            }
        })
        .buffer_unordered(conf.validator_hosts.len())
        .filter_map(|res| async { res })
        .collect::<Vec<R>>()
        .await;

    Ok(results)
}

/// Generic function to perform a POST request to a validator.
async fn request_validator_signature<'a, P, R>(
    host: &str,
    endpoint: &str,
    payload: P,
    context: &str,
) -> Result<Option<R>>
// eyre::Result
where
    P: TryInto<Bytes> + Copy,
    <P as TryInto<Bytes>>::Error: Into<eyre::Report>,
    R: DeserializeOwned,
{
    info!(
        "Dymension, requesting {} sigs from validator: {}",
        context, host
    );
    let client = reqwest::Client::new();
    let url = format!("{}{}", host, endpoint);

    let body_bytes = payload.try_into().map_err(Into::into)?;

    let res = client.post(url).body(body_bytes).send().await?;

    let status = res.status();
    if status == StatusCode::OK {
        let body = res.json::<R>().await?;
        Ok(Some(body))
    } else {
        Err(eyre!(
            "Failed to request signature for {}: validator at {} returned status {}",
            context,
            host,
            status
        ))
    }
}
