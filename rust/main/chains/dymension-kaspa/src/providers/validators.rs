use crate::ConnectionConf;
use hyperlane_core::ChainResult;

// ===================================================================
//
//  Part 1: The "Data-Only" Struct
//
//  `ValidatorsClient` is now a simple data container. It has NO
//  async methods in its `impl` block. This makes it trivial for the
//  Rust compiler to prove that it is `Send` and `Sync`, which is the
//  core fix for the downstream errors in your other modules.
//
// ===================================================================

#[derive(Debug, Clone)]
pub struct ValidatorsClient {
    pub conf: ConnectionConf,
}

impl ValidatorsClient {
    /// Returns a new Rpc Provider.
    pub fn new(conf: ConnectionConf) -> ChainResult<Self> {
        Ok(ValidatorsClient { conf })
    }

    /// Returns the configured multisig threshold for the ISM.
    pub fn multisig_threshold_hub_ism(&self) -> usize {
        self.conf.multisig_threshold_hub_ism
    }
}


// ===================================================================
//
//  Part 2: The Asynchronous API Logic
//
//  All the async logic now lives inside this separate, public `api`
//  module. These are free-standing functions that operate on the
//  `ConnectionConf` data.
//
//  This clean separation of data (above) from behavior (below)
//  is what solves the compilation problem safely.
//
//  To use these functions elsewhere, you will call them like:
//  `validators::api::get_deposit_sigs(&client.conf, &fxg).await`
//
// ===================================================================

pub mod api {
    use super::ConnectionConf;
    use crate::endpoints::*;
    use bytes::Bytes;
    use dym_kas_core::{confirmation::ConfirmationFXG, deposit::DepositFXG, withdraw::WithdrawFXG};
    use eyre::{eyre, Result};
    use futures::stream::{self, StreamExt};
    use hyperlane_core::{ChainResult, Signature, SignedCheckpointWithMessageId};
    use kaspa_wallet_pskt::prelude::Bundle;
    use reqwest::StatusCode;
    use serde::de::DeserializeOwned;
    use tracing::{error, info};

    /// this runs on relayer
    pub async fn get_deposit_sigs(
        conf: &ConnectionConf,
        fxg: &DepositFXG,
    ) -> ChainResult<Vec<SignedCheckpointWithMessageId>> {
        get_signatures_from_validators(conf, fxg, ROUTE_VALIDATE_NEW_DEPOSITS, "deposit").await
    }

    /// this runs on relayer
    pub async fn get_confirmation_sigs(
        conf: &ConnectionConf,
        fxg: &ConfirmationFXG,
    ) -> ChainResult<Vec<Signature>> {
        get_signatures_from_validators(conf, fxg, ROUTE_VALIDATE_CONFIRMED_WITHDRAWALS, "confirmation").await
    }

    /// this runs on relayer
    pub async fn get_withdraw_sigs(
        conf: &ConnectionConf,
        fxg: &WithdrawFXG,
    ) -> ChainResult<Vec<Bundle>> {
        get_signatures_from_validators(conf, fxg, ROUTE_SIGN_PSKTS, "withdrawal").await
    }

    /// Generic function to request signatures from all validators in parallel.
    async fn get_signatures_from_validators<'a, P, R>(
        conf: &ConnectionConf,
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
                        info!("Dymension, got {} sig response ok, validator: {:?}", context, h);
                        Some(sig)
                    }
                    Ok(None) => {
                        error!("Dymension, got {} sig response None, validator: {:?}", context, h);
                        None
                    }
                    Err(e) => {
                        error!("Dymension, got {} sig response Err, validator: {:?}, error: {:?}", context, h, e);
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
    where
        P: TryInto<Bytes> + Copy,
        <P as TryInto<Bytes>>::Error: Into<eyre::Report>,
        R: DeserializeOwned,
    {
        info!("Dymension, requesting {} sigs from validator: {}", context, host);
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
}