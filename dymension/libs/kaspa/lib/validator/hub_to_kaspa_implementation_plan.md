# Hub->Kaspa Validator Implementation Plan (G() Function)

## Overview

This document outlines the implementation plan for the G() function in the **hub->kaspa unescrow flow**, specifically step 5 in the validator workflow:

```
5. Validator call G(batch of PSKT<Signer>) to get Ok(true) [no validations]
6. Validator signs to get batch of PSKT<Combiner>, return to relayer
```

## Current State Analysis

### Existing Components

1. **Relayer F() Function**: Located in `dymension/libs/kaspa/lib/relayer/src/hub_to_kaspa.rs`
   - Function: `build_withdrawal_pskts()`
   - Returns: `Result<Option<PSKT<Signer>>>`
   - Purpose: Constructs PSKT for withdrawal transactions from hub withdrawal messages

2. **Validator Library**: Located in `dymension/libs/kaspa/lib/validator/src/`
   - Current modules: `withdraw.rs`, `deposit.rs`, `confirmation.rs`, `signer.rs`, `lib.rs`
   - Existing functionality: `sign_escrow_spend()` in `withdraw.rs` handles PSKT signing
   - The G() function is going to be located in `withdrawal.rs`. Method `validate_withdrawals`.

3. **Core Library**: Located in `dymension/libs/kaspa/lib/core/src/`
   - Shared entities: `Escrow`, `MessageIDs`, payload handling
   - Common utilities for PSKT manipulation

### Input/Output Analysis

**Input to G()**: 
- `WithdrawFXG`, i.e. `batch of PSKT<Signer>`, from relayer F() function
- `WithdrawFXG` contains a bundle with PSKTs with withdrawal transaction details and message IDs in proprietaries

**Expected Output from G()**:
- `Ok(true)` with no validations (as per task specification)
- Side effect: Sign PSKTs to produce `batch of PSKT<Combiner>`
- Further, it will be wired with the relayer

## Implementation Plan

### Phase 1: Define API Interface

#### 1.1 Create G() Function Signature -> already exists

The G() function is going to be located in `withdrawal.rs`. Method `validate_withdrawals`
```rust
pub async fn validate_withdrawals(fxg: &WithdrawFXG) -> Result<bool> {
    Ok(true)
}
```

### Phase 2: Core Implementation

#### 2.2 Validation Logic (Minimal per Spec)
According to the task specification: "no validations", but we should include:
- Basic PSKT structure validation (ensure it's properly formed)
- Escrow key availability check
- Input/output sanity checks (non-zero amounts, valid addresses)

#### 2.3 Signing Process
- Leverage existing `sign_escrow_spend()` function from `withdraw.rs`
- Process each PSKT in the batch individually
- Combine signatures using existing multisig logic
- Return vector of `PSKT<Combiner>`

### Phase 3: Integration Points

#### 3.1 HTTP API Endpoint
- Add endpoint to validator HTTP server: `/api/v1/process-withdrawal-batch`
- Accept JSON payload with base64-encoded PSKTs
- Return JSON response with signed PSKTs

#### 3.2 Error Handling
```rust
#[derive(Debug, thiserror::Error)]
pub enum WithdrawalProcessingError {
    #[error("Invalid PSKT structure: {0}")]
    InvalidPskt(String),
    
    #[error("Signing failed: {0}")]
    SigningError(String),
    
    #[error("Escrow key unavailable")]
    EscrowKeyUnavailable,
}
```

#### 3.3 Logging and Metrics
- Log withdrawal batch processing events
- Metrics for successful/failed validations
- PSKT signing duration metrics

### Phase 4: Testing Strategy

#### 4.1 Unit Tests
- Test G() function with valid PSKT batches
- Test error conditions (malformed PSKTs, missing keys)
- Test signing functionality with mock escrow

#### 4.2 Integration Tests
- End-to-end test with relayer F() -> validator G() flow
- Test with real Kaspa network (testnet)
- Validate PSKT<Combiner> can be finalized and broadcast

### Phase 5: Configuration and Deployment

#### 5.1 Configuration Updates
- Add hub->kaspa validator configuration options
- Escrow private key management
- HTTP server endpoint configuration

#### 5.2 Documentation
- API documentation for G() function endpoint
- Integration guide for relayer->validator communication
- Error handling and troubleshooting guide

## File Structure Changes

```
dymension/libs/kaspa/lib/validator/src/
├── lib.rs                     # Updated exports
├── hub_to_kaspa.rs           # NEW: G() function implementation
├── withdraw.rs               # Enhanced with batch processing
├── api/                      # NEW: HTTP API handlers
│   └── withdrawal.rs
└── errors.rs                 # NEW: Error types
```

## Dependencies to Add

In `validator/Cargo.toml`:
```toml
# HTTP server (if not already present)
axum = { workspace = true }
tower = { workspace = true }

# Async utilities
futures = { workspace = true }

# Error handling
thiserror = "1.0"
```

## Security Considerations

1. **Private Key Management**: Ensure escrow private keys are securely stored and accessed
2. **Input Validation**: Validate PSKT structure even with "no validations" requirement
3. **Rate Limiting**: Implement rate limiting on the HTTP endpoint
4. **Audit Logging**: Log all withdrawal processing for auditability

## Success Criteria

1. G() function successfully processes batches of PSKT<Signer>
2. Returns Ok(true) for all valid inputs per specification
3. Produces valid PSKT<Combiner> that can be finalized by relayer
4. Integration tests pass with relayer F() function
5. HTTP API endpoint functional and documented
6. Performance acceptable for expected throughput

## Implementation Timeline

- **Phase 1**: 1 day - API design and type definitions
- **Phase 2**: 2-3 days - Core G() function implementation
- **Phase 3**: 1-2 days - HTTP API and integration
- **Phase 4**: 2 days - Testing and validation
- **Phase 5**: 1 day - Documentation and deployment prep

**Total Estimated Time**: 7-9 days

## Next Steps

1. Review and approve this implementation plan
2. Begin Phase 1 implementation
3. Create initial stub functions and tests
4. Implement core G() function logic
5. Add HTTP API layer
6. Comprehensive testing and validation 