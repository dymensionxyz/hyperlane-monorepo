use std::sync::Arc;

use solana_client::nonblocking::rpc_client::RpcClient;

use crate::client::SealevelRpcClient;

// Modern Agave nodes serialize `InstructionError::BorshIoError` as a bare
// string, which the pinned solana-transaction-status fork must tolerate
// (dymensionxyz/solana tag hyperlane-1.14.13-dym-2026-08-16). A single such
// failed transaction used to make the whole getBlock response unreadable with
// `invalid type: unit variant, expected newtype variant`, permanently jamming
// indexer cursors on that block.
#[test]
fn parses_block_containing_modern_agave_transaction_error() {
    let block = serde_json::json!({
        "previousBlockhash": "11111111111111111111111111111111",
        "blockhash": "11111111111111111111111111111111",
        "parentSlot": 364279161,
        "blockTime": 1723800000,
        "blockHeight": 342000000,
        "transactions": [{
            "transaction": ["AQ==", "base64"],
            "meta": {
                "err": { "InstructionError": [2, "BorshIoError"] },
                "status": { "Err": { "InstructionError": [2, "BorshIoError"] } },
                "fee": 5000,
                "preBalances": [1000],
                "postBalances": [995],
            },
        }],
    });

    let block: solana_transaction_status::UiConfirmedBlock = serde_json::from_value(block).unwrap();

    let meta = block.transactions.unwrap()[0].meta.clone().unwrap();
    assert!(meta.err.is_some());
    assert!(meta.status.is_err());
}

//#[tokio::test]
async fn _test_get_block() {
    let rpc_client = RpcClient::new("<solana-rpc>".to_string());
    // given
    let client = SealevelRpcClient::from_rpc_client(Arc::new(rpc_client));

    // when
    let slot = 301337842; // block which requires latest version of solana-client
    let result = client.get_block(slot).await;

    // then
    assert!(result.is_ok());
}
