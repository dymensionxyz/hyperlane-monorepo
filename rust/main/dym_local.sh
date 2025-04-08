#!/bin/bash

set -e # Exit immediately if a command exits with a non-zero status.
# set -x # Uncomment for verbose command execution tracing

echo "--- Starting Solana Native Warp Route Dispatch Script (using cargo run) ---"

# --- Configuration ---
# Borrowed heavily from src/sealevel/solana.rs constants

# Versions
SOLANA_CONTRACTS_CLI_VERSION="1.14.20" # For building programs
SOLANA_CONTRACTS_CLI_RELEASE_URL="github.com/solana-labs/solana"
SOLANA_NETWORK_CLI_VERSION="2.0.24"   # For running the validator (agave)
SOLANA_NETWORK_CLI_RELEASE_URL="github.com/anza-xyz/agave"

# Paths (relative to script execution directory: hyperlane-monorepo/rust/main)
REPO_ROOT=$(git rev-parse --show-toplevel)
SEALEVEL_DIR="$REPO_ROOT/rust/sealevel"
TS_INFRA_DIR="$REPO_ROOT/typescript/infra"
MAIN_DIR="$REPO_ROOT/rust/main"
# CLIENT_BIN="$SEALEVEL_DIR/target/release/hyperlane-sealevel-client" # No longer needed
SEALEVEL_CLI_DIR="$SEALEVEL_DIR/cli"
SEALEVEL_CLI_CARGO_TOML="$SEALEVEL_CLI_DIR/Cargo.toml"
SBF_OUT_PATH="$MAIN_DIR/target/dist/solana-sbf" # Where built .so files will go
SOLANA_PROGRAM_LIBRARY_ARCHIVE="https://github.com/hyperlane-xyz/solana-program-library/releases/download/2024-08-23/spl.tar.gz"

# Solana Programs (subset needed)
SOLANA_HYPERLANE_PROGRAMS=(
    "mailbox"
    # "validator-announce" # Not needed
    # "ism/multisig-ism-message-id" # Not needed
    "hyperlane-sealevel-token"
    "hyperlane-sealevel-token-native"
    # "hyperlane-sealevel-token-collateral" # Not needed for native
    "hyperlane-sealevel-igp" # Still needed for core deploy usually
)
SPL_PROGRAMS=(
    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA spl_token.so"
    "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb spl_token_2022.so"
    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL spl_associated_token_account.so"
    "noopb9bkMVfRPU8AsbpTUg8AQkHtKwMYZiFUjNRtMmV spl_noop.so"
)

# Keys & Configs (use absolute paths derived from REPO_ROOT for safety)
SOLANA_DEPLOYER_KEYPAIR_ABS="$SEALEVEL_DIR/environments/local-e2e/accounts/test_deployer-keypair.json"
SOLANA_DEPLOYER_ACCOUNT_ABS="$SEALEVEL_DIR/environments/local-e2e/accounts/test_deployer-account.json"
SOLANA_WARPROUTE_TOKEN_CONFIG_FILE_ABS="$SEALEVEL_DIR/environments/local-e2e/warp-routes/testwarproute/token-config.json"
SOLANA_CHAIN_CONFIG_FILE_ABS="$SEALEVEL_DIR/environments/local-e2e/chain-config.json"
SOLANA_GAS_ORACLE_CONFIG_FILE_ABS="$SEALEVEL_DIR/environments/local-e2e/gas-oracle-configs.json" # May not be strictly needed if skipping IGP configure
SOLANA_ENVS_DIR_ABS="$SEALEVEL_DIR/environments"
SOLANA_ENV_NAME="local-e2e"

# Test Chain IDs
SOLANA_LOCAL_DOMAIN="13375" # Local Solana chain domain ID
DUMMY_REMOTE_DOMAIN="9999"  # Doesn't matter, just needs a destination

# Addresses / IDs (adjust if deployment changes, but these are likely stable for local-e2e env)
MAILBOX_PROGRAM_ID="C44B7BCygXVvR1B94pEZt3aXraQ5evMer11uan4VvzC9" # From local-e2e/sealeveltest1/chain-config.json
# IGP_PROGRAM_ID="GwHaw8ewMyzZn9vvrZEnTEAAYpLdkGYs195XWcLDCN4U"     # Will be deployed but unused

# Transfer details
TRANSFER_AMOUNT="1000000" # Lamports (0.001 SOL)
DUMMY_RECIPIENT_ADDR="0x1234567890123456789012345678901234567890" # Just needs to be a valid hex address format


# Temp directories
INSTALL_DIR=$(mktemp -d)
TOOLS_BUILD_DIR="$INSTALL_DIR/solana-contracts-cli"
TOOLS_RUN_DIR="$INSTALL_DIR/solana-network-cli"
SOLANA_LEDGER_DIR=$(mktemp -d)

# Cleanup function
cleanup() {
  echo "--- Cleaning up ---"
  # Kill validator if running
  if [ -n "$VALIDATOR_PID" ] && ps -p $VALIDATOR_PID > /dev/null; then
     echo "Killing Solana validator (PID: $VALIDATOR_PID)..."
     kill "$VALIDATOR_PID" || echo "Failed to kill validator, may have already exited."
     wait "$VALIDATOR_PID" 2>/dev/null || true
  fi
  echo "Removing temporary directories..."
  rm -rf "$INSTALL_DIR"
  rm -rf "$SOLANA_LEDGER_DIR"
  # Consider keeping SBF_OUT_PATH for faster rebuilds if script is run often
  # rm -rf "$SBF_OUT_PATH"
  echo "--- Cleanup Complete ---"
}
trap cleanup EXIT

# Helper function to run direct solana commands
run_solana_cmd() {
    local solana_bin_path="$1"
    local solana_config_path="$2"
    shift 2
    echo "Executing: PATH=$solana_bin_path:\$PATH solana --config \"$solana_config_path\" \"$@\""
    PATH="$solana_bin_path:$PATH" solana --config "$solana_config_path" "$@"
}

# Helper function to run the hyperlane client via 'cargo run'
run_hl_client_cargo() {
    # $1 = Solana RUN bin path (needed for PATH if client calls solana internally, though likely not needed)
    local solana_bin_path="$1"
    local solana_config_path="$2"
    shift 2
    # Prepend fixed args for cargo run and the client itself
    echo "Executing: cargo run --release --manifest-path \"$SEALEVEL_CLI_CARGO_TOML\" -- --config \"$solana_config_path\" --keypair \"$SOLANA_DEPLOYER_KEYPAIR_ABS\" \"$@\""
    # Set PATH just in case the client forks a solana process, though unlikely
    PATH="$solana_bin_path:$PATH" \
    cargo run --release --manifest-path "$SEALEVEL_CLI_CARGO_TOML" -- \
        --config "$solana_config_path" \
        --keypair "$SOLANA_DEPLOYER_KEYPAIR_ABS" \
        "$@"
}


# --- 1. Install Solana Tools ---

echo "--- Installing Solana CLI tools ---"
mkdir -p "$TOOLS_BUILD_DIR" "$TOOLS_RUN_DIR"

if [ ! -x "$TOOLS_BUILD_DIR/bin/solana" ]; then
    echo "Downloading Contracts CLI..."
    curl --location --output "$INSTALL_DIR/solana-contracts.tar.bz2" \
        "https://github.com/solana-labs/solana/releases/download/v$SOLANA_CONTRACTS_CLI_VERSION/solana-release-x86_64-unknown-linux-gnu.tar.bz2"
    tar --extract --file "$INSTALL_DIR/solana-contracts.tar.bz2" --directory "$INSTALL_DIR"
    mv "$INSTALL_DIR/solana-release"/* "$TOOLS_BUILD_DIR/"
    rm "$INSTALL_DIR/solana-contracts.tar.bz2"
fi
SOLANA_BUILD_BIN_PATH="$TOOLS_BUILD_DIR/bin"

if [ ! -x "$TOOLS_RUN_DIR/bin/solana" ]; then
    echo "Downloading Network CLI..."
    curl --location --output "$INSTALL_DIR/solana-network.tar.bz2" \
        "https://github.com/anza-xyz/agave/releases/download/v$SOLANA_NETWORK_CLI_VERSION/agave-release-x86_64-unknown-linux-gnu.tar.bz2"
    tar --extract --file "$INSTALL_DIR/solana-network.tar.bz2" --directory "$INSTALL_DIR"
    mv "$INSTALL_DIR/agave-release"/* "$TOOLS_RUN_DIR/"
    rm "$INSTALL_DIR/solana-network.tar.bz2"
fi
SOLANA_RUN_BIN_PATH="$TOOLS_RUN_DIR/bin"

echo "Solana tools installed/verified."
echo "Build tools: $SOLANA_BUILD_BIN_PATH"
echo "Run tools: $SOLANA_RUN_BIN_PATH"


# --- 2. Build Hyperlane Solana Programs ---

echo "--- Building Hyperlane Solana programs ---"
mkdir -p "$SBF_OUT_PATH"

# Check if SPL programs exist, download if not
if [ ! -f "$SBF_OUT_PATH/spl_token.so" ]; then
    echo "Downloading SPL source..."
    curl --location --output "$SBF_OUT_PATH/spl.tar.gz" "$SOLANA_PROGRAM_LIBRARY_ARCHIVE"
    tar --extract --file "$SBF_OUT_PATH/spl.tar.gz" --directory "$SBF_OUT_PATH"
    rm "$SBF_OUT_PATH/spl.tar.gz"
fi

# Build Hyperlane programs
CARGO_SBF_BIN="$SOLANA_BUILD_BIN_PATH/cargo-build-sbf"
for program_path in "${SOLANA_HYPERLANE_PROGRAMS[@]}"; do
    # Simple check to avoid rebuild if .so exists
    program_name=$(basename "$program_path" | sed 's/-/_/g') # Convert hyphens for filename
    if [ ! -f "$SBF_OUT_PATH/${program_name}.so" ]; then
        echo "Building $program_path..."
        (cd "$SEALEVEL_DIR/programs/$program_path" && \
         SBF_OUT_PATH="$SBF_OUT_PATH" "$CARGO_SBF_BIN" --sbf-out-dir "$SBF_OUT_PATH")
    else
        echo "Skipping build for $program_path (.so exists)"
    fi
done
echo "Hyperlane Solana programs built to $SBF_OUT_PATH."


# --- 3. Start Solana Test Validator ---

echo "--- Starting Solana Test Validator ---"
SOLANA_CONFIG_FILE="$INSTALL_DIR/solana-cli-config.yaml"
touch "$SOLANA_CONFIG_FILE"

# Construct validator arguments
VALIDATOR_ARGS=(
    "--quiet"
    "--reset"
    "--ledger" "$SOLANA_LEDGER_DIR"
    # Fund the deployer account
    "--account" "E9VrvAdGRvCguN2XgXsgu9PNmMM3vZsU8LSUrM68j8ty" "$SOLANA_DEPLOYER_ACCOUNT_ABS"
)
# Add SPL programs
for program_info in "${SPL_PROGRAMS[@]}"; do
    read -r address lib <<< "$program_info"
    VALIDATOR_ARGS+=( "--bpf-program" "$address" "$SBF_OUT_PATH/$lib" )
done

# Start validator in background
echo "Starting validator..."
PATH="$SOLANA_RUN_BIN_PATH:$PATH" solana-test-validator "${VALIDATOR_ARGS[@]}" > "$INSTALL_DIR/validator.log" 2>&1 &
VALIDATOR_PID=$!
echo "Solana validator started (PID: $VALIDATOR_PID), ledger: $SOLANA_LEDGER_DIR. Log: $INSTALL_DIR/validator.log. Waiting for it to initialize..."

# Wait for validator RPC to be available
MAX_WAIT=30
COUNT=0
while ! run_solana_cmd "$SOLANA_RUN_BIN_PATH" "$SOLANA_CONFIG_FILE" block-height > /dev/null 2>&1; do
    sleep 1
    COUNT=$((COUNT + 1))
    if [ $COUNT -ge $MAX_WAIT ]; then
        echo "Validator failed to start within $MAX_WAIT seconds. Check logs: $INSTALL_DIR/validator.log"
        exit 1
    fi
    echo -n "."
done
echo " Validator RPC is up."

# Set Solana CLI config URL (redundant if already done, but safe)
run_solana_cmd "$SOLANA_RUN_BIN_PATH" "$SOLANA_CONFIG_FILE" config set --url localhost
echo "Solana CLI configured for localhost."


# --- 4. Deploy Hyperlane Core Contracts ---

echo "--- Deploying Hyperlane Core contracts ---"
# We only deploy for one chain ('sealeveltest1' config from local-e2e)
run_hl_client_cargo "$SOLANA_RUN_BIN_PATH" "$SOLANA_CONFIG_FILE" \
    core deploy \
    --environment "$SOLANA_ENV_NAME" \
    --environments-dir "$SOLANA_ENVS_DIR_ABS" \
    --built-so-dir "$SBF_OUT_PATH" \
    --chain sealeveltest1 \
    --local-domain "$SOLANA_LOCAL_DOMAIN" \
    --yes # Auto-confirm deployment prompts

echo "Hyperlane Core deployed."


# --- 5. Deploy Native Warp Route ---

echo "--- Deploying Native Warp Route ---"
# Deploy the warp route defined in the local-e2e config under 'testwarproute'
DEPLOY_OUTPUT=$(run_hl_client_cargo "$SOLANA_RUN_BIN_PATH" "$SOLANA_CONFIG_FILE" \
    warp-route deploy \
    --environment "$SOLANA_ENV_NAME" \
    --environments-dir "$SOLANA_ENVS_DIR_ABS" \
    --built-so-dir "$SBF_OUT_PATH" \
    --chain sealeveltest1 \
    --warp-route-name testwarproute \
    --token-config-file "$SOLANA_WARPROUTE_TOKEN_CONFIG_FILE_ABS" \
    --chain-config-file "$SOLANA_CHAIN_CONFIG_FILE_ABS" \
    --ata-payer-funding-amount 1000000000 \
    --yes) # Auto-confirm deployment prompts

echo "$DEPLOY_OUTPUT"

# Extract the deployed Warp Route Program ID
WARP_ROUTE_PROGRAM_ID=$(echo "$DEPLOY_OUTPUT" | grep -oP 'Program ID: \K[1-9A-HJ-NP-Za-km-z]+' | tail -n 1)
if [ -z "$WARP_ROUTE_PROGRAM_ID" ]; then
    echo "Error: Could not extract Warp Route Program ID from deployment output."
    exit 1
fi
echo "Native Warp Route deployed. Program ID: $WARP_ROUTE_PROGRAM_ID"


# --- 6. Initiate Transfer Remote ---

echo "--- Initiating Native Token Transfer Remote ---"
TRANSFER_OUTPUT=$(run_hl_client_cargo "$SOLANA_RUN_BIN_PATH" "$SOLANA_CONFIG_FILE" \
    token transfer-remote \
    "$SOLANA_DEPLOYER_KEYPAIR_ABS" \
    "$TRANSFER_AMOUNT" \
    "$DUMMY_REMOTE_DOMAIN" \
    "$DUMMY_RECIPIENT_ADDR" \
    native \
    --program-id "$WARP_ROUTE_PROGRAM_ID")

echo "$TRANSFER_OUTPUT"

# Extract the message ID (requires specific log format from client/program)
MESSAGE_ID=$(echo "$TRANSFER_OUTPUT" | grep -oP 'Message ID: \K0x[0-9a-fA-F]+' | head -n 1) # Adjust grep pattern if log changes
# Fallback regex from e2e tests if the above fails
if [ -z "$MESSAGE_ID" ]; then
    MESSAGE_ID=$(echo "$TRANSFER_OUTPUT" | grep -oP 'Dispatched message to \d+, ID \K0x[0-9a-fA-F]+' | head -n 1)
fi

if [ -z "$MESSAGE_ID" ]; then
    echo "Error: Could not extract Message ID from transfer output."
    echo "Check Solana logs for dispatch: PATH=$SOLANA_RUN_BIN_PATH:\$PATH solana logs --config \"$SOLANA_CONFIG_FILE\""
    exit 1
fi
echo "Transfer initiated. Message ID: $MESSAGE_ID"


# --- Success ---
echo ""
echo "--- Script finished successfully! ---"
echo "A native token transfer message ($MESSAGE_ID) should have been dispatched via the Mailbox ($MAILBOX_PROGRAM_ID)."
echo "You can inspect the state using:"
echo "PATH=$SOLANA_RUN_BIN_PATH:\$PATH solana account $MAILBOX_PROGRAM_ID --config \"$SOLANA_CONFIG_FILE\""
echo "PATH=$SOLANA_RUN_BIN_PATH:\$PATH solana logs --config \"$SOLANA_CONFIG_FILE\""
echo "Keep the validator running (PID: $VALIDATOR_PID) or press Ctrl+C to stop it and clean up."

# Keep script running to allow manual inspection, otherwise validator gets killed by trap
wait $VALIDATOR_PID
