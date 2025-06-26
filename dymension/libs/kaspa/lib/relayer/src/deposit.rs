use crate::handle_new_deposit;
use core::api::deposits::Deposit;
use core::deposit::DepositFXG;
use std::error::Error;
use tokio::runtime::Runtime;
use eyre::Result;

pub async fn on_new_deposit(deposit: &Deposit) -> Result<Option<DepositFXG>> {
    let deposit_tx_result = handle_new_deposit(deposit.id.to_string()).await?;
    Ok(Some(deposit_tx_result))
}
