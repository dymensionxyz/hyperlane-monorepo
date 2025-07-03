# Hub->Kaspa Validator Implementation Plan (G() Function)

## Overview

This document outlines the implementation plan for the G() function in the **hub->kaspa unescrow flow**.

## Current State Analysis

### Existing Components

1. **Relayer F() Function**: Located in `dymension/libs/kaspa/lib/relayer/src/hub_to_kaspa.rs`
   - Function: `build_withdrawal_pskt()`
   - Purpose: Constructs PSKT for withdrawal transactions from hub withdrawal messages

2. **Validator Library**: Located in `dymension/libs/kaspa/lib/validator/src/`
   - Current modules: `withdraw.rs`, `deposit.rs`, `confirmation.rs`, `signer.rs`, `lib.rs`
   - The G() function is located in `withdraw.rs`. Function `validate_withdrawals`.

3. **Core Library**: Located in `dymension/libs/kaspa/lib/core/src/`
   - Shared entities: `Escrow`, `MessageIDs`, payload handling

### Input/Output Analysis

**Input to G()**: 
- `PKST<Signer>` and a list of respective `HyperlaneMessage`

**Expected Output from G()**:
- `Ok(bool)` repending on the result 

## Implementation Plan

### Phase 1: Create G() Function Signature -> already exists

The G() function is going to be located in `withdrawal.rs`. Method `validate_withdrawals`
```rust
pub async fn validate_withdrawals(fxg: &WithdrawFXG) -> Result<bool> {
    Ok(true)
}
```

### Phase 2: Core Implementation

In `validate_withdrawals`:

- For every message, check that it is delivered using the `CosmosGrpcClient.delivered` method: `rust/main/chains/hyperlane-cosmos-native/src/providers/grpc.rs`. If any of them isn't, return `false`.
- Query the last outpoint from the Hub. Use `CosmosGrpcClient.withdrawal_status`: `rust/main/chains/hyperlane-cosmos-native/src/providers/grpc.rs`. This fetches the last outpoint and checks statuses of the messages. All messages should be `WithdrawalStatus::Unprocessed` (ref: `dymension/libs/kaspa/lib/relayer/src/hub_to_kaspa.rs:442`). If any of them isn't, return `false`.
- In PSKT, iterate over inputs and check that it contains the Hub outpoint. If it isn't, return `false`.
- Check that UTXO outputs actully align with withdrawals. For every message, get its `TokenMessage` (ref: `dymension/libs/kaspa/lib/relayer/src/withdraw/withdraw_construction.rs:44`) to find the recepient and amount. Each pair should be reflected in outputs. Note that we might have duplicate pairs if one address transfers the same amount twice (eg, "user1" transfers 10 KAS twice, so we have two HL messages with the same `TokenMessage`). Outputs shouldn't have any extra items expept for: escrow change, relayer change, and respective HL messages. If any of HL messages are not present in outputs OR outputs have items are not present in HL messages (except for change outputs), return `false`.
- ??Avoid for now?? -> The proposed kaspa generated TXs are a linked sequence – Now we assume that we have only one tx, but still need to impl this flow.
