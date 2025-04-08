//! The message explorer scraper is responsible for building and maintaining a
//! relational database of the Hyperlane state across blockchains to empower us and
//! our users to trace and debug messages and other system state.
//!
//! Information scrapped is predominately recoverable simply be re-scraping the
//! blockchains, however, they may be some additional "enrichment" which is only
//! practically discoverable at the time it was recorded. This additional
//! information is not critical to the functioning of the system.
//!
//! One scraper instance is run per chain and together they will be able to
//! piece together the full hyperlane system state in the relational database.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use agent::Scraper;
use eyre::Result;
use hyperlane_base::agent_main;

mod agent;
mod conversions;
mod date_time;
mod db;
mod settings;
mod store;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Logging is not initialised at this point, so, using `println!`
    println!("Scraper agent starting up...");

    agent_main::<Scraper>().await;

    let rpc_client = SealevelFallbackRpcClient::new(rpc_url);
    let domain = HyperlaneDomain::new(domain_id);
    let contract_addresses = vec![];
    let conf = ConnectionConf::new(native_token, contract_addresses);

    let provider = SealevelProvider::new(rpc_client, domain, &contract_addresses, &conf);

    let tx_submitter = Box::new(SubmitSealevelRpc::new(provider.clone()));

    let ixer =
        SealevelMailboxIndexer::new(provider, tx_submitter, locator, conf, advanced_log_meta);

    let res = ixer.get_dispatched_message_with_nonce(nonce).await?;

    println!("res: {:?}", res);

    Ok(())
}
