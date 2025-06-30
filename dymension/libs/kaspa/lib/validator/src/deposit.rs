use core::deposit::DepositFXG;

use eyre::Result;
use kaspa_wrpc_client::KaspaRpcClient;

use crate::validate_deposit;

pub async fn validate_new_deposit(client: &KaspaRpcClient, deposit: &DepositFXG) -> Result<bool> {
    let validation_result = validate_deposit(client, deposit).await?; 
    Ok(validation_result)
}