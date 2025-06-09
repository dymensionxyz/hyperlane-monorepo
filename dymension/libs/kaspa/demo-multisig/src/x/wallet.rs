#![allow(unused)] // TODO: remove

use kaspa_core::info;
use kaspa_wallet_core::api::WalletApi;
use kaspa_wallet_core::error::Error;
use kaspa_wallet_core::wallet::Wallet;
use kaspa_wallet_keys::secret::Secret;

use kaspa_wallet_core::prelude::*; // Import the prelude for easy access to traits/structs

use std::sync::Arc;

use crate::x::consts::*;
use kaspa_wrpc_client::Resolver;

pub async fn get_wallet(s: &Secret) -> Result<Arc<Wallet>, Error> {
    let w = Arc::new(Wallet::try_new(
        Wallet::local_store()?,
        Some(Resolver::default()),
        Some(NETWORK_ID),
    )?);

    // Start background services (UTXO processor, event handling).
    w.start().await?;

    w.clone()
        .connect(Some(URL.to_string()), &NETWORK_ID)
        .await?;

    w.clone().wallet_open(s, None, true, false).await?;

    let accounts = w.clone().accounts_enumerate().await?;
    let account_descriptor = accounts.get(0).ok_or("Wallet has no accounts.")?;
    info!("Found account: {:?}", account_descriptor.name_or_id());

    let account_id = account_descriptor.account_id;
    w.clone().accounts_select(Some(account_id)).await?;
    w.clone().accounts_activate(Some(vec![account_id])).await?;
    let account = w.clone().account()?;

    Ok(w)
}

pub async fn debug_balance(wallet: Arc<Wallet>) -> Result<(), Error> {
    let a = wallet.account()?;
    for _ in 0..10 {
        if a.balance().is_some() {
            break;
        }
        workflow_core::task::sleep(std::time::Duration::from_millis(200)).await;
    }

    if let Some(b) = a.balance() {
        info!("Account Balance:");
        info!("  Mature:   {} KAS", sompi_to_kaspa_string(b.mature));
        info!("  Pending:  {} KAS", sompi_to_kaspa_string(b.pending));
        info!("  Outgoing: {} KAS", sompi_to_kaspa_string(b.outgoing));
    } else {
        info!("Account has no balance or is still syncing.");
    }

    wallet.stop().await?;

    Ok(())
}
