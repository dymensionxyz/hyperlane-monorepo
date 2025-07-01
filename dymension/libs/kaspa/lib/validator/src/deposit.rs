use core::deposit::DepositFXG;
use std::sync::Arc;

use eyre::Result;
use kaspa_wallet_core::prelude::DynRpcApi;

use crate::validate_deposit;

pub async fn validate_new_deposit(client: &Arc<DynRpcApi>, deposit: &DepositFXG, escrow_address: &str) -> Result<bool> {
    let validation_result = validate_deposit(client, deposit, escrow_address).await?;
    Ok(validation_result)
}
