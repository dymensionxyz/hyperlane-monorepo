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

// Real v1 transaction from mainnet slot 449391489 (2026-09-22). Guards the
// pinned solana-transaction-status fork against the v1 message shape
// (`transactionConfig` present, `addressTableLookups` absent).
#[test]
fn parses_block_containing_transaction_v1() {
    let block: solana_transaction_status::UiConfirmedBlock = serde_json::from_str(
        r#"{"previousBlockhash":"8DpX17FK7k4W8nJzq7srhAUQEZDhNten2N79Tz4amX3D","blockhash":"GyJRCzk2WG1W4ccXQft2AYz8UvGx6pMTyhfr2Ec9ETgZ","parentSlot":449391488,"blockTime":1790083456,"blockHeight":427431888,"transactions":[{"meta":{"computeUnitsConsumed":13,"costUnits":1351,"err":{"InstructionError":[0,{"Custom":2}]},"fee":5003,"innerInstructions":[],"loadedAddresses":{"readonly":[],"writable":[]},"logMessages":["Program W1LDCARDa67SPBG7TFpQivHnEZXRtxCFP13ysEd1bWR invoke [1]","Program W1LDCARDa67SPBG7TFpQivHnEZXRtxCFP13ysEd1bWR consumed 13 of 19 compute units","Program W1LDCARDa67SPBG7TFpQivHnEZXRtxCFP13ysEd1bWR failed: custom program error: 0x2"],"postBalances":[588942741,1781760,1141440],"postTokenBalances":[],"preBalances":[588947744,1781760,1141440],"preTokenBalances":[],"rewards":null,"status":{"Err":{"InstructionError":[0,{"Custom":2}]}}},"transaction":{"message":{"accountKeys":["updftC18GBD8tzneHb87mf5FyZJUZ9YMcS6PgEiSWmB","Cr7GWeCZbsQfbtkYUVCdCfJNQvKUV9rZ9weptryRU3ds","W1LDCARDa67SPBG7TFpQivHnEZXRtxCFP13ysEd1bWR"],"header":{"numReadonlySignedAccounts":0,"numReadonlyUnsignedAccounts":1,"numRequiredSignatures":1},"instructions":[{"accounts":[1,0],"data":"11111111HZFQq3buqbBsd6z353WcSiaWUvo25kG89owyR1QH45CT","programIdIndex":2,"stackHeight":1}],"recentBlockhash":"2CNcgcc1reqPQrKq4PGaVMSHkgbYpikW1pMuYxK6kMVG","transactionConfig":{"computeUnitLimit":19,"heapSize":null,"loadedAccountsDataSizeLimit":32000,"priorityFee":3}},"signatures":["MuiQSVoHuB94adeqQvrFqeaQFJFTYo6EGc696KWuUHMKGyzcvqvnuCRhvf55ZWSBKsjWoZQFPCpaZTFbuhKrgyE"]},"version":1}]}"#,
    )
    .unwrap();

    let tx = &block.transactions.unwrap()[0];
    assert_eq!(
        tx.version,
        Some(solana_sdk::transaction::TransactionVersion::Number(1))
    );
    assert!(tx.meta.is_some());
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
