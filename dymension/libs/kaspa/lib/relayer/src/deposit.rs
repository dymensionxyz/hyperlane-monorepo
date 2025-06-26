use core::api::deposits::Deposit;
use core::deposit::DepositFXG;
use std::error::Error;
use tokio::runtime::Runtime;
use crate::handle_new_deposit;

pub fn on_new_deposit(deposit: &Deposit) -> Option<DepositFXG> {

    // Create a new Tokio runtime
    let rt = Runtime::new().unwrap();

    // Block the current thread and execute the async handle_new_deposit function
    let deposit_tx_result: Result<DepositFXG, Box<dyn Error>> = rt.block_on(async {
        handle_new_deposit(deposit.id.to_string()).await
    });

    match deposit_tx_result {
        Ok(deposit) => {
            return Some(deposit)
        }
        Err(e) => {
            eprintln!("Error processing deposit: {}", e);
        }
    }
    None
}
