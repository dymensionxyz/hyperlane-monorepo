use super::conf::ValidatorStuff;
use super::endpoints::*;
use super::providers::KaspaProvider;
use axum::{
    body::Bytes,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::post,
    Router,
};
use dym_kas_core::api::client::HttpClient;
use dym_kas_core::deposit::DepositFXG;
use dym_kas_core::escrow::EscrowPublic;
use dym_kas_core::wallet::EasyKaspaWallet;
use dym_kas_core::{confirmation::ConfirmationFXG, withdraw::WithdrawFXG};
use dym_kas_validator::confirmation::validate_confirmed_withdrawals;
use dym_kas_validator::deposit::{validate_new_deposit, MustMatch as DepositMustMatch};
use dym_kas_validator::error::ValidationError;
use dym_kas_validator::withdraw::{validate_sign_withdrawal_fxg, MustMatch as WithdrawMustMatch};
pub use dym_kas_validator::KaspaSecpKeypair;
use eyre::Report;
use hyperlane_core::{
    Checkpoint, CheckpointWithMessageId, HyperlaneSignerExt, Signable,
    SignedCheckpointWithMessageId, H256,
};
use hyperlane_core::{HyperlaneChain, HyperlaneDomain, Signature as HLCoreSignature};
use hyperlane_cosmos::{native::ModuleQueryClient, CosmosProvider};
use hyperlane_cosmos_rs::dymensionxyz::dymension::kas::ProgressIndication;
use hyperlane_cosmos_rs::prost::Message;
use kaspa_wallet_core::prelude::DynRpcApi;
use kaspa_wallet_pskt::prelude::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha3::{digest::Update, Digest, Keccak256};
use std::sync::Arc;
use tracing::{error, info};

#[derive(Clone)]
pub struct ValidatorISMSigningResources<
    S: HyperlaneSignerExt + Send + Sync + 'static,
    H: HyperlaneSignerExt + Clone + Send + Sync + 'static,
> {
    direct_signer: Arc<S>,
    singleton_signer: H,
}

impl<
        S: HyperlaneSignerExt + Send + Sync + 'static,
        H: HyperlaneSignerExt + Clone + Send + Sync + 'static,
    > ValidatorISMSigningResources<S, H>
{
    pub fn new(direct_signer: Arc<S>, singleton_signer: H) -> Self {
        Self {
            direct_signer,
            singleton_signer,
        }
    }

    pub async fn sign_with_fallback<T: Signable + Send + Clone>(
        &self,
        signable: T,
    ) -> Result<hyperlane_core::SignedType<T>, eyre::Report> {
        const RETRIES: usize = 5;
        const RETRY_DELAY_MS: u64 = 100;

        for attempt in 0..RETRIES {
            match self.direct_signer.sign(signable.clone()).await {
                Ok(signed) => {
                    tracing::debug!(attempt, "Signed with direct signer");
                    return Ok(signed);
                }
                Err(_err) => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                }
            }
        }

        Ok(self.singleton_signer.sign(signable).await?)
    }
}

enum AppError {
    Validation(ValidationError),
    Generic(eyre::Report),
}

impl From<ValidationError> for AppError {
    fn from(e: ValidationError) -> Self {
        AppError::Validation(e)
    }
}

impl From<eyre::Report> for AppError {
    fn from(e: eyre::Report) -> Self {
        AppError::Generic(e)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::Validation(ve) => {
                eprintln!("Validator error: {:?}", ve);

                // Map ValidationError variants to HTTP status codes
                let (status_code, response_body) = match ve {
                    // 202 ACCEPTED: Transaction not final yet (retryable with wait)
                    ValidationError::NotSafeAgainstReorg { .. } => (
                        StatusCode::ACCEPTED,
                        format!("Deposit not final: {}", ve),
                    ),

                    // 503 SERVICE UNAVAILABLE: Infrastructure/external service issues (retryable with fixed delay)
                    ValidationError::HubNotBootstrapped
                    | ValidationError::HubQueryError { .. }
                    | ValidationError::KaspaNodeError { .. }
                    | ValidationError::ExternalApiError { .. } => (
                        StatusCode::SERVICE_UNAVAILABLE,
                        format!("Service unavailable: {}", ve),
                    ),

                    // 422 UNPROCESSABLE ENTITY: Validation failed, transaction is invalid (non-retryable)
                    ValidationError::DoubleSpending { .. }
                    | ValidationError::HLMessageFieldMismatch { .. }
                    | ValidationError::InvalidTransactionHash
                    | ValidationError::PayloadParseError { .. }
                    | ValidationError::InsufficientDepositAmount { .. }
                    | ValidationError::WrongDepositAddress { .. }
                    | ValidationError::HLMessageIdMismatch
                    | ValidationError::EscrowWithdrawalNotAllowed { .. }
                    | ValidationError::PayloadMismatch
                    | ValidationError::MessageIdsMismatch
                    | ValidationError::EscrowAmountMismatch { .. }
                    | ValidationError::SigHashType
                    | ValidationError::InvalidOutpointData { .. }
                    | ValidationError::MissingTransactionPayload
                    | ValidationError::MissingTransactionInputs
                    | ValidationError::MissingTransactionOutputs
                    | ValidationError::MissingScriptPubKeyAddress
                    | ValidationError::ScriptPubKeyExtractionError { .. }
                    | ValidationError::AnchorSpent { .. }
                    | ValidationError::NonEscrowAnchor { .. }
                    | ValidationError::MultipleAnchors => (
                        StatusCode::UNPROCESSABLE_ENTITY,
                        format!("Transaction rejected: {}", ve),
                    ),

                    // 404 NOT FOUND: Resource not found (may be retryable)
                    ValidationError::MessageNotDispatched { .. }
                    | ValidationError::TransactionDataNotFound
                    | ValidationError::UtxoNotFound { .. }
                    | ValidationError::OutpointMissing { .. }
                    | ValidationError::PreviousTransactionNotFound
                    | ValidationError::NextAnchorNotFound
                    | ValidationError::AnchorNotFound { .. }
                    | ValidationError::NoMessages => (
                        StatusCode::NOT_FOUND,
                        format!("Required data not found: {}", ve),
                    ),

                    // 409 CONFLICT: State conflict (retryable)
                    ValidationError::MessagesNotUnprocessed
                    | ValidationError::HubAnchorMismatch { .. }
                    | ValidationError::AnchorMismatch { .. } => (
                        StatusCode::CONFLICT,
                        format!("State conflict: {}", ve),
                    ),

                    // 500 INTERNAL SERVER ERROR: Conversion/unexpected errors
                    ValidationError::BlockHashConversionError { .. }
                    | ValidationError::TransactionHashConversionError { .. }
                    | ValidationError::TransactionFetchError { .. }
                    | ValidationError::FinalityCheckError { .. }
                    | ValidationError::FailedGeneralVerification { .. }
                    | ValidationError::InsufficientOutpoints { .. }
                    | ValidationError::MessageCacheLengthMismatch { .. }
                    | ValidationError::MissingOutputs => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Internal error: {}", ve),
                    ),
                };

                (status_code, response_body).into_response()
            }
            AppError::Generic(e) => {
                let err_msg = e.to_string();
                eprintln!("Validator error: {}", err_msg);

                // For generic errors, check if they're WebSocket/infrastructure related
                let (status_code, response_body) = if err_msg.contains("WebSocket is not connected")
                    || err_msg.contains("RPC Server (remote error)")
                    || err_msg.contains("reconnection failed")
                    || err_msg.contains("sign")
                {
                    (
                        StatusCode::SERVICE_UNAVAILABLE,
                        format!("Service unavailable: {}", err_msg),
                    )
                } else {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Internal error: {}", err_msg),
                    )
                };

                (status_code, response_body).into_response()
            }
        }
    }
}

type HandlerResult<T> = Result<T, AppError>;

pub fn router<
    S: HyperlaneSignerExt + Send + Sync + 'static,
    H: HyperlaneSignerExt + Clone + Send + Sync + 'static,
>(
    resources: ValidatorServerResources<S, H>,
) -> Router {
    Router::new()
        .route(
            ROUTE_VALIDATE_NEW_DEPOSITS,
            post(respond_validate_new_deposits::<S, H>),
        )
        .route(
            ROUTE_VALIDATE_CONFIRMED_WITHDRAWALS,
            post(respond_validate_confirmed_withdrawals::<S, H>),
        )
        .route(ROUTE_SIGN_PSKTS, post(respond_sign_pskts::<S, H>))
        .route("/kaspa-ping", post(respond_kaspa_ping::<S, H>))
        .with_state(Arc::new(resources))
}

async fn respond_kaspa_ping<
    S: HyperlaneSignerExt + Send + Sync + 'static,
    H: HyperlaneSignerExt + Clone + Send + Sync + 'static,
>(
    State(_): State<Arc<ValidatorServerResources<S, H>>>,
    _body: Bytes,
) -> HandlerResult<Json<String>> {
    error!("validator server: got kaspa ping");
    Ok(Json("pong".to_string()))
}

#[derive(Clone)]
pub struct ValidatorServerResources<
    S: HyperlaneSignerExt + Send + Sync + 'static,
    H: HyperlaneSignerExt + Clone + Send + Sync + 'static,
> {
    signing: Option<ValidatorISMSigningResources<S, H>>,
    kas_provider: Option<Box<KaspaProvider>>,
}

impl<
        S: HyperlaneSignerExt + Send + Sync + 'static,
        H: HyperlaneSignerExt + Clone + Send + Sync + 'static,
    > ValidatorServerResources<S, H>
{
    pub fn new(
        signing: ValidatorISMSigningResources<S, H>,
        kas_provider: Box<KaspaProvider>,
    ) -> Self {
        Self {
            signing: Some(signing),
            kas_provider: Some(kas_provider),
        }
    }
    fn must_signing(&self) -> &ValidatorISMSigningResources<S, H> {
        self.signing.as_ref().unwrap()
    }
    fn kas_key_source(&self) -> crate::conf::KaspaEscrowKeySource {
        self.kas_provider.as_ref().unwrap().kas_key_source().clone()
    }
    fn must_api(&self) -> Arc<DynRpcApi> {
        self.must_wallet().api()
    }

    fn must_escrow(&self) -> EscrowPublic {
        self.kas_provider.as_ref().unwrap().escrow()
    }

    fn must_wallet(&self) -> &EasyKaspaWallet {
        self.kas_provider.as_ref().unwrap().wallet()
    }

    fn must_hub_rpc(&self) -> &CosmosProvider<ModuleQueryClient> {
        self.kas_provider.as_ref().unwrap().hub_rpc()
    }

    pub fn must_kas_domain(&self) -> &HyperlaneDomain {
        self.kas_provider.as_ref().unwrap().domain()
    }

    fn must_rest_client(&self) -> &HttpClient {
        &self.kas_provider.as_ref().unwrap().rest().client.client
    }

    fn must_val_stuff(&self) -> &ValidatorStuff {
        self.kas_provider.as_ref().unwrap().must_validator_stuff()
    }
}

impl<
        S: HyperlaneSignerExt + Send + Sync + 'static,
        H: HyperlaneSignerExt + Clone + Send + Sync + 'static,
    > Default for ValidatorServerResources<S, H>
{
    fn default() -> Self {
        Self {
            signing: None,
            kas_provider: None,
        }
    }
}

async fn respond_validate_new_deposits<
    S: HyperlaneSignerExt + Send + Sync + 'static,
    H: HyperlaneSignerExt + Clone + Send + Sync + 'static,
>(
    State(res): State<Arc<ValidatorServerResources<S, H>>>,
    body: Bytes,
) -> HandlerResult<Json<SignedCheckpointWithMessageId>> {
    info!("validator: checking new kaspa deposit");
    let deposits: DepositFXG = body.try_into().map_err(|e: eyre::Report| AppError::Generic(e))?;
    if res.must_val_stuff().toggles.deposit_enabled {
        validate_new_deposit(
            &res.must_api(),
            res.must_rest_client(),
            &deposits,
            &res.must_wallet().net,
            &res.must_escrow().addr,
            res.must_hub_rpc(),
            DepositMustMatch::new(
                res.must_val_stuff().hub_domain,
                res.must_val_stuff().hub_token_id,
                res.must_val_stuff().kas_domain,
                res.must_val_stuff().kas_token_placeholder,
            ),
        )
<<<<<<< Updated upstream
        .await
        .map_err(|e| {
            eprintln!("Deposit validation failed: {:?}", e);
            AppError(Report::from(e))
        })?;
=======
        .await;

        // If validation failed with a WebSocket error, attempt reconnection and retry
        match validation_result {
            Ok(_) => {}
            Err(e) => {
                // Check if it's a KaspaNodeError (WebSocket disconnection)
                let should_reconnect = matches!(e, ValidationError::KaspaNodeError { .. });

                if should_reconnect {
                    tracing::warn!(
                        error = ?e,
                        "Deposit validation failed with Kaspa node error, attempting reconnection"
                    );

                    // Attempt to reconnect
                    if let Err(reconnect_err) = res.must_wallet().reconnect().await {
                        return Err(AppError::Generic(eyre::eyre!(
                            "Deposit validation failed and reconnection failed: original error: {}, reconnect error: {}",
                            e,
                            reconnect_err
                        )));
                    }

                    // Retry validation after reconnection
                    validate_new_deposit(
                        &res.must_api(),
                        res.must_rest_client(),
                        &deposits,
                        &res.must_wallet().net,
                        &res.must_escrow().addr,
                        res.must_hub_rpc(),
                        DepositMustMatch::new(
                            res.must_val_stuff().hub_domain,
                            res.must_val_stuff().hub_token_id,
                            res.must_val_stuff().kas_domain,
                            res.must_val_stuff().kas_token_placeholder,
                        ),
                    )
                    .await
                    .map_err(|retry_err| {
                        eprintln!("Deposit validation failed after reconnection: {:?}", retry_err);
                        AppError::Validation(retry_err)
                    })?;
                } else {
                    eprintln!("Deposit validation failed: {:?}", e);
                    return Err(AppError::Validation(e));
                }
            }
        }
>>>>>>> Stashed changes
    }
    info!(
        message_id = ?deposits.hl_message.id(),
        "validator: deposit is valid"
    );

    let msg_id = deposits.hl_message.id();
    let domain = deposits.hl_message.origin;

    let zero_array = [0u8; 32];
    let to_sign: CheckpointWithMessageId = CheckpointWithMessageId {
        checkpoint: Checkpoint {
            mailbox_domain: domain,
            merkle_tree_hook_address: H256::from_slice(&zero_array),
            root: H256::from_slice(&zero_array),
            index: 0,
        },
        message_id: msg_id,
    };

    let sig = res
        .must_signing()
        .sign_with_fallback(to_sign)
        .await
        .map_err(|e| AppError::Generic(e.into()))?;
    info!("validator: signed deposit");

    Ok(Json(sig))
}

async fn respond_sign_pskts<
    S: HyperlaneSignerExt + Send + Sync + 'static,
    H: HyperlaneSignerExt + Clone + Send + Sync + 'static,
>(
    State(res): State<Arc<ValidatorServerResources<S, H>>>,
    body: Bytes,
) -> HandlerResult<Json<Bundle>> {
    info!("validator: signing pskts");

    let fxg: WithdrawFXG = body.try_into().map_err(|e: Report| AppError::Generic(e))?;
    let escrow = res.must_escrow();
    let val_stuff = res.must_val_stuff();

    let kas_key_source = res.kas_key_source().clone();

    let bundle = validate_sign_withdrawal_fxg(
        fxg,
        val_stuff.toggles.withdrawal_enabled,
        res.must_hub_rpc().query(),
        escrow,
        || async move {
            match &kas_key_source {
                crate::conf::KaspaEscrowKeySource::Direct(json_str) => {
                    serde_json::from_str(json_str)
                        .map_err(|e| eyre::eyre!("parse Kaspa keypair from JSON: {}", e))
                }
                crate::conf::KaspaEscrowKeySource::Aws(aws_config) => {
                    dym_kas_kms::load_kaspa_keypair_from_aws(aws_config)
                        .await
                        .map_err(|e| eyre::eyre!("load Kaspa keypair from AWS: {}", e))
                }
            }
        },
        WithdrawMustMatch::new(
            res.must_wallet().net.address_prefix,
            res.must_escrow(),
            val_stuff.hub_domain,
            val_stuff.hub_token_id,
            val_stuff.kas_domain,
            val_stuff.kas_token_placeholder,
            val_stuff.hub_mailbox_id.clone(),
        ),
    )
    .await
    .map_err(|e| {
        eprintln!("Withdrawal validation and singing failed: {:?}", e);
        AppError::Generic(Report::from(e))
    })?;

    Ok(Json(bundle))
}

#[derive(Clone)]
pub struct SignableProgressIndication {
    progress_indication: ProgressIndication,
}

impl Serialize for SignableProgressIndication {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encoded = self.progress_indication.encode_to_vec();
        serializer.serialize_bytes(&encoded)
    }
}

impl<'de> Deserialize<'de> for SignableProgressIndication {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = Bytes::deserialize(deserializer)?;
        let progress_indication =
            ProgressIndication::decode(bytes.as_ref()).map_err(serde::de::Error::custom)?;
        Ok(SignableProgressIndication {
            progress_indication,
        })
    }
}

impl Signable for SignableProgressIndication {
    fn signing_hash(&self) -> H256 {
        // Byte derivation matches Hub code: https://github.com/dymensionxyz/dymension/blob/main/x/kas/types/signing.go
        let mut bz = vec![];
        bz.extend(
            self.progress_indication
                .old_outpoint
                .clone()
                .unwrap()
                .transaction_id,
        );
        bz.extend(
            self.progress_indication
                .old_outpoint
                .clone()
                .unwrap()
                .index
                .to_be_bytes(),
        );
        bz.extend(
            self.progress_indication
                .new_outpoint
                .clone()
                .unwrap()
                .transaction_id,
        );
        bz.extend(
            self.progress_indication
                .new_outpoint
                .clone()
                .unwrap()
                .index
                .to_be_bytes(),
        );
        for w in self.progress_indication.processed_withdrawals.clone() {
            bz.extend(w.message_id.as_bytes());
        }
        H256::from_slice(Keccak256::new().chain(bz).finalize().as_slice())
    }
}

async fn respond_validate_confirmed_withdrawals<
    S: HyperlaneSignerExt + Send + Sync + 'static,
    H: HyperlaneSignerExt + Clone + Send + Sync + 'static,
>(
    State(res): State<Arc<ValidatorServerResources<S, H>>>,
    body: Bytes,
) -> HandlerResult<Json<HLCoreSignature>> {
    info!("validator: checking confirmed kaspa withdrawal");
    let conf_fxg: ConfirmationFXG = body.try_into().map_err(|e: eyre::Report| AppError::Generic(e))?;

    if res.must_val_stuff().toggles.withdrawal_confirmation_enabled {
        validate_confirmed_withdrawals(&conf_fxg, res.must_rest_client(), &res.must_escrow().addr)
            .await
            .map_err(|e| {
                eprintln!("Withdrawal confirmation validation failed: {:?}", e);
                AppError::Validation(e)
            })?;
        info!("validator: confirmed withdrawal is valid");
    }

    let progress_indication = &conf_fxg.progress_indication;

    let sig = res
        .must_signing()
        .sign_with_fallback(SignableProgressIndication {
            progress_indication: progress_indication.clone(),
        })
        .await
        .map_err(|e| AppError::Generic(e.into()))?;

    info!("validator: signed confirmed withdrawal");

    Ok(Json(sig.signature))
}
