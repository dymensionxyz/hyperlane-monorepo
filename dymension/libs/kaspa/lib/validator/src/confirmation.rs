use corelib::confirmation::ConfirmationFXG;

use eyre::Result;

pub async fn validate_confirmed_withdrawals(fxg: &ConfirmationFXG) -> Result<bool> {
    // TODO:
    // validate correctnerss of new anchor utxo
    // validate the correctness of the withdrawals
    //
    Ok(true)
}
