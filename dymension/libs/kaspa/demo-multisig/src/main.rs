use kaspa_consensus_core::{
    hashing::sighash::{calc_schnorr_signature_hash, SigHashReusedValuesUnsync},
    tx::{TransactionId, TransactionOutpoint, UtxoEntry},
};
use kaspa_txscript::{multisig_redeem_script, opcodes::codes::OpData65, pay_to_script_hash_script, script_builder::ScriptBuilder};
use kaspa_wallet_pskt::prelude::{
    Combiner, Creator, Extractor, Finalizer, Inner, InputBuilder, OutputBuilder, SignInputOk, Signature, Signer, Updater, PSKT,
};
use secp256k1::{rand::thread_rng, Keypair};
use std::{iter, str::FromStr};

// Return an rpc client for testnet 10
fn get_testnet_client(){

}

struct EscrowInfo {
    keypairs: Vec<Keypair>,
    redeem_script: Vec<u8>,
    escrow_address: String,
}

fn create_escrow_addr() -> EscrowInfo {
    let kps = [
        Keypair::new(secp256k1::SECP256K1, &mut thread_rng()),
        Keypair::new(secp256k1::SECP256K1, &mut thread_rng()),
    ];
    
    let redeem_script = multisig_redeem_script(
        kps.iter().map(|pk| pk.x_only_public_key().0.serialize()),
        2
    ).unwrap();
    
    let script_pub_key = pay_to_script_hash_script(&redeem_script);
    let escrow_address = format!("{:?}", script_pub_key);

    EscrowInfo {
        keypairs: kps.to_vec(),
        redeem_script,
        escrow_address,
    }
}

fn deposit_funds(
    escrow_address: &str,
    amount: u64,
    from_utxo: UtxoEntry,
    from_outpoint: TransactionOutpoint,
    from_privkey: [u8; 32],
) -> Result<(), String> {
    let input = InputBuilder::default()
        .utxo_entry(from_utxo)
        .previous_outpoint(from_outpoint)
        .sig_op_count(1)
        .build()
        .unwrap();

    let output = OutputBuilder::default()
        .amount(amount)
        .script_public_key(escrow_address.parse().unwrap())
        .build()
        .unwrap();

    let pskt = PSKT::<Creator>::default()
        .inputs_modifiable()
        .outputs_modifiable()
        .constructor()
        .input(input)
        .output(output)
        .updater()
        .set_sequence(u64::MAX, 0)
        .unwrap()
        .signer();

    let schnorr_key = secp256k1::Keypair::from_seckey_slice(secp256k1::SECP256K1, &from_privkey)
        .map_err(|e| e.to_string())?;

    let reused_values = SigHashReusedValuesUnsync::new();
    let signed_pskt = pskt
        .pass_signature_sync(|tx, sighash| -> Result<Vec<SignInputOk>, String> {
            tx.tx
                .inputs
                .iter()
                .enumerate()
                .map(|(idx, _input)| {
                    let hash = calc_schnorr_signature_hash(&tx.as_verifiable(), idx, sighash[idx], &reused_values);
                    let msg = secp256k1::Message::from_digest_slice(hash.as_bytes().as_slice()).unwrap();
                    Ok(SignInputOk {
                        signature: Signature::Schnorr(schnorr_key.sign_schnorr(msg)),
                        pub_key: schnorr_key.public_key(),
                        key_source: None,
                    })
                })
                .collect()
        })
        .map_err(|e| e.to_string())?;

    let finalizer = signed_pskt.finalizer();
    let finalizer = finalizer.finalize_sync(|inner: &Inner| -> Result<Vec<Vec<u8>>, String> {
        Ok(inner
            .inputs
            .iter()
            .map(|input| -> Vec<u8> {
                let sig = input.partial_sigs.get(&schnorr_key.public_key()).unwrap();
                iter::once(OpData65)
                    .chain(sig.into_bytes())
                    .chain([input.sighash_type.to_u8()])
                    .collect()
            })
            .collect())
    }).map_err(|e| e.to_string())?;

    let extractor = finalizer.extractor().map_err(|e| e.to_string())?;
    let tx = extractor.extract_tx().map_err(|e| e.to_string())?(10).0;

    println!("Deposit transaction ready to submit: {}", serde_json::to_string_pretty(&tx).unwrap());
    Ok(())
}

fn create_tx(
    escrow_info: &EscrowInfo,
    utxo: UtxoEntry,
    outpoint: TransactionOutpoint,
    destination_address: String,
    amount: u64,
) -> PSKT<Signer> {
    let input = InputBuilder::default()
        .utxo_entry(utxo)
        .previous_outpoint(outpoint)
        .sig_op_count(2)
        .redeem_script(escrow_info.redeem_script.clone())
        .build()
        .unwrap();

    let output = OutputBuilder::default()
        .amount(amount)
        .script_public_key(destination_address.parse().unwrap())
        .build()
        .unwrap();

    PSKT::<Creator>::default()
        .inputs_modifiable()
        .outputs_modifiable()
        .constructor()
        .input(input)
        .output(output)
        .updater()
        .set_sequence(u64::MAX, 0)
        .unwrap()
        .signer()
}

fn get_sigs(pskt: PSKT<Signer>, escrow_info: &EscrowInfo) -> Vec<PSKT<Signer>> {
    let reused_values = SigHashReusedValuesUnsync::new();
    let sign = |signer_pskt: PSKT<Signer>, kp: &Keypair| {
        signer_pskt
            .pass_signature_sync(|tx, sighash| -> Result<Vec<SignInputOk>, String> {
                tx.tx
                    .inputs
                    .iter()
                    .enumerate()
                    .map(|(idx, _input)| {
                        let hash = calc_schnorr_signature_hash(&tx.as_verifiable(), idx, sighash[idx], &reused_values);
                        let msg = secp256k1::Message::from_digest_slice(hash.as_bytes().as_slice()).unwrap();
                        Ok(SignInputOk {
                            signature: Signature::Schnorr(kp.sign_schnorr(msg)),
                            pub_key: kp.public_key(),
                            key_source: None,
                        })
                    })
                    .collect()
            })
            .unwrap()
    };

    escrow_info.keypairs.iter()
        .map(|kp| sign(pskt.clone(), kp))
        .collect()
}

fn submit_tx(signed_pskts: Vec<PSKT<Signer>>) -> Result<(), String> {
    let mut combined = signed_pskts[0].clone().combiner();
    for pskt in signed_pskts.iter().skip(1) {
        combined = (combined + pskt.clone().combiner()).map_err(|e| e.to_string())?;
    }

    let finalizer = combined.finalizer();
    let finalizer = finalizer.finalize_sync(|inner: &Inner| -> Result<Vec<Vec<u8>>, String> {
        Ok(inner
            .inputs
            .iter()
            .map(|input| -> Vec<u8> {
                let signatures: Vec<_> = input
                    .partial_sigs
                    .iter()
                    .flat_map(|(_, sig)| {
                        iter::once(OpData65)
                            .chain(sig.into_bytes())
                            .chain([input.sighash_type.to_u8()])
                    })
                    .collect();

                signatures
                    .into_iter()
                    .chain(
                        ScriptBuilder::new()
                            .add_data(input.redeem_script.as_ref().unwrap().as_slice())
                            .unwrap()
                            .drain()
                            .iter()
                            .cloned(),
                    )
                    .collect()
            })
            .collect())
    }).map_err(|e| e.to_string())?;

    let extractor = finalizer.extractor().map_err(|e| e.to_string())?;
    let tx = extractor.extract_tx().map_err(|e| e.to_string())?(10).0;
    
    println!("Transaction ready to submit: {}", serde_json::to_string_pretty(&tx).unwrap());
    Ok(())
}

fn run_demo() {
    // Create escrow info
    let escrow_info = create_escrow_addr();
    println!("Escrow address: {}", escrow_info.escrow_address);

    // Deposit funds to escrow
    let from_utxo = UtxoEntry {
        amount: 2_000_000_000, // 2 KAS
        script_public_key: "your_testnet_script_pubkey".parse().unwrap(),
        block_daa_score: 0,
        is_coinbase: false,
    };
    let from_outpoint = TransactionOutpoint {
        transaction_id: TransactionId::from_str("your_testnet_tx_id").unwrap(),
        index: 0,
    };
    let from_privkey = [0u8; 32]; // Replace with your testnet private key

    deposit_funds(
        &escrow_info.escrow_address,
        1_000_000_000, // 1 KAS
        from_utxo,
        from_outpoint,
        from_privkey,
    ).unwrap();

    // Wait for deposit to confirm...

    // Create transaction to spend from escrow
    let utxo = UtxoEntry {
        amount: 1_000_000_000, // 1 KAS
        script_public_key: pay_to_script_hash_script(&escrow_info.redeem_script),
        block_daa_score: 0,
        is_coinbase: false,
    };
    let outpoint = TransactionOutpoint {
        transaction_id: TransactionId::from_str("escrow_tx_id").unwrap(), // Replace with actual escrow tx id
        index: 0,
    };

    let pskt = create_tx(
        &escrow_info,
        utxo,
        outpoint,
        "kaspa:qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq".to_string(),
        900_000_000, // 0.9 KAS
    );

    let signed_pskts = get_sigs(pskt, &escrow_info);
    submit_tx(signed_pskts).unwrap();
}

fn main() {
    run_demo();
} 

fn example_multisig(){
    let kps = [Keypair::new(secp256k1::SECP256K1, &mut thread_rng()), Keypair::new(secp256k1::SECP256K1, &mut thread_rng())];
    let redeem_script = multisig_redeem_script(kps.iter().map(|pk| pk.x_only_public_key().0.serialize()), 2).unwrap();
    // Create the PSKT.
    let created = PSKT::<Creator>::default().inputs_modifiable().outputs_modifiable();
    let ser = serde_json::to_string_pretty(&created).expect("Failed to serialize after creation");
    println!("Serialized after creation: {}", ser);

    // The first constructor entity receives the PSKT and adds an input.
    let pskt: PSKT<Creator> = serde_json::from_str(&ser).expect("Failed to deserialize");
    // let in_0 = dummy_out_point();
    let input_0 = InputBuilder::default()
        .utxo_entry(UtxoEntry {
            amount: 12793000000000,
            script_public_key: pay_to_script_hash_script(&redeem_script),
            block_daa_score: 36151168,
            is_coinbase: false,
        })
        .previous_outpoint(TransactionOutpoint {
            transaction_id: TransactionId::from_str("63020db736215f8b1105a9281f7bcbb6473d965ecc45bb2fb5da59bd35e6ff84").unwrap(),
            index: 0,
        })
        .sig_op_count(2)
        .redeem_script(redeem_script)
        .build()
        .unwrap();
    let pskt_in0 = pskt.constructor().input(input_0);
    let ser_in_0 = serde_json::to_string_pretty(&pskt_in0).expect("Failed to serialize after adding first input");
    println!("Serialized after adding first input: {}", ser_in_0);

    let combiner_pskt: PSKT<Combiner> = serde_json::from_str(&ser).expect("Failed to deserialize");
    let combined_pskt = (combiner_pskt + pskt_in0).unwrap();
    let ser_combined = serde_json::to_string_pretty(&combined_pskt).expect("Failed to serialize after adding output");
    println!("Serialized after combining: {}", ser_combined);

    // The PSKT is now ready for handling with the updater role.
    let updater_pskt: PSKT<Updater> = serde_json::from_str(&ser_combined).expect("Failed to deserialize");
    let updater_pskt = updater_pskt.set_sequence(u64::MAX, 0).expect("Failed to set sequence");
    let ser_updated = serde_json::to_string_pretty(&updater_pskt).expect("Failed to serialize after setting sequence");
    println!("Serialized after setting sequence: {}", ser_updated);

    let signer_pskt: PSKT<Signer> = serde_json::from_str(&ser_updated).expect("Failed to deserialize");
    let reused_values = SigHashReusedValuesUnsync::new();
    let sign = |signer_pskt: PSKT<Signer>, kp: &Keypair| {
        signer_pskt
            .pass_signature_sync(|tx, sighash| -> Result<Vec<SignInputOk>, String> {
                let tx = dbg!(tx);
                tx.tx
                    .inputs
                    .iter()
                    .enumerate()
                    .map(|(idx, _input)| {
                        let hash = calc_schnorr_signature_hash(&tx.as_verifiable(), idx, sighash[idx], &reused_values);
                        let msg = secp256k1::Message::from_digest_slice(hash.as_bytes().as_slice()).unwrap();
                        Ok(SignInputOk {
                            signature: Signature::Schnorr(kp.sign_schnorr(msg)),
                            pub_key: kp.public_key(),
                            key_source: None,
                        })
                    })
                    .collect()
            })
            .unwrap()
    };
    let signed_0 = sign(signer_pskt.clone(), &kps[0]);
    let signed_1 = sign(signer_pskt, &kps[1]);
    let combiner_pskt: PSKT<Combiner> = serde_json::from_str(&ser_updated).expect("Failed to deserialize");
    let combined_signed = (combiner_pskt + signed_0).and_then(|combined| combined + signed_1).unwrap();
    let ser_combined_signed = serde_json::to_string_pretty(&combined_signed).expect("Failed to serialize after combining signed");
    println!("Combined Signed: {}", ser_combined_signed);
    let pskt_finalizer: PSKT<Finalizer> = serde_json::from_str(&ser_combined_signed).expect("Failed to deserialize");
    let pskt_finalizer = pskt_finalizer
        .finalize_sync(|inner: &Inner| -> Result<Vec<Vec<u8>>, String> {
            Ok(inner
                .inputs
                .iter()
                .map(|input| -> Vec<u8> {
                    // todo actually required count can be retrieved from redeem_script, sigs can be taken from partial sigs according to required count
                    // considering xpubs sorted order

                    let signatures: Vec<_> = kps
                        .iter()
                        .flat_map(|kp| {
                            let sig = input.partial_sigs.get(&kp.public_key()).unwrap().into_bytes();
                            iter::once(OpData65).chain(sig).chain([input.sighash_type.to_u8()])
                        })
                        .collect();
                    signatures
                        .into_iter()
                        .chain(
                            ScriptBuilder::new()
                                .add_data(input.redeem_script.as_ref().unwrap().as_slice())
                                .unwrap()
                                .drain()
                                .iter()
                                .cloned(),
                        )
                        .collect()
                })
                .collect())
        })
        .unwrap();
    let ser_finalized = serde_json::to_string_pretty(&pskt_finalizer).expect("Failed to serialize after finalizing");
    println!("Finalized: {}", ser_finalized);

    let extractor_pskt: PSKT<Extractor> = serde_json::from_str(&ser_finalized).expect("Failed to deserialize");
    let tx = extractor_pskt.extract_tx().unwrap()(10).0;
    let ser_tx = serde_json::to_string_pretty(&tx).unwrap();
    println!("Tx: {}", ser_tx);
}
