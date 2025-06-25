use anyhow::Result;
use core::wallet::EasyKaspaWallet;
use core::withdraw::WithdrawFXG;
use hyperlane_core::HyperlaneMessage;
use kaspa_wallet_pskt::prelude::Bundle;
use kaspa_wallet_pskt::prelude::*;

/// Updated signature matching the specification
pub async fn on_new_withdrawals(
    messages: Vec<HyperlaneMessage>,
    w: EasyKaspaWallet,
    // and cosmos provider
) -> Result<Option<WithdrawFXG>> {
    // TODO: impl
    let v: Vec<PSKT<Signer>> = vec![];
    let fxg = WithdrawFXG::new(Bundle::from(v));
    Ok(Some(fxg))
}
