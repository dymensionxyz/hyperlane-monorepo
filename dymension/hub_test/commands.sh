# Q: What is this?
# A: Some commands to run Dymension Hub + Anvil instance and connect them and relay between them
# Scenario: Dymension Hub will have collateral ADYM and Anvil will have synthetic memo

##############################################################################################3
# STEP: Start chains and deploy contracts

################
# HUB: 

BASE_PATH="/Users/danwt/Documents/dym/d-dymension/scripts/hyperlane_test"
source $BASE_PATH/env.sh

cd dymension/
bash scripts/setup_local.sh
dymd start --log_level=debug

HUB_DOMAIN=1260813472 
ETH_DOMAIN=31337

# create noop ism
hub tx hyperlane ism create-noop "${HUB_FLAGS[@]}"
ISM=$(curl -s http://localhost:1318/hyperlane/v1/isms | jq '.isms.[0].id' -r); echo $ISM;

# create mailbox
# ism, local domain
hub tx hyperlane mailbox create  $ISM $HUB_DOMAIN "${HUB_FLAGS[@]}"
MAILBOX=$(curl -s http://localhost:1318/hyperlane/v1/mailboxes   | jq '.mailboxes.[0].id' -r); echo $MAILBOX;
# TODO: set addresses.yaml

# create noop hook
hub tx hyperlane hooks noop create "${HUB_FLAGS[@]}"
NOOP_HOOK=$(curl -s http://localhost:1318/hyperlane/v1/noop_hooks | jq '.noop_hooks.[0].id' -r); echo $NOOP_HOOK;

# create merkle hook
hub tx hyperlane hooks merkle create $MAILBOX "${HUB_FLAGS[@]}"
MERKLE_HOOK=$(curl -s http://localhost:1318/hyperlane/v1/merkle_tree_hooks | jq '.merkle_tree_hooks.[0].id' -r); echo $MERKLE_HOOK;
# TODO: set addresses.yaml

# TODO: IGP needed? Gas config?!! (don't think so, for this test)

# update mailbox
# mailbox, default hook (e.g. IGP), required hook (e.g. merkle tree)
hub tx hyperlane mailbox set $MAILBOX --default-hook $NOOP_HOOK --required-hook $MERKLE_HOOK "${HUB_FLAGS[@]}"

DENOM="adym"
hub tx hyperlane-transfer dym-create-collateral-token $MAILBOX $DENOM "${HUB_FLAGS[@]}"
TOKEN_ID=$(curl -s http://localhost:1318/hyperlane/v1/tokens | jq '.tokens.[0].id' -r); echo $TOKEN_ID
# TODO: set foreignDeployment in warp config

################
# ANVIL: 

anvil --port 8545 --chain-id 31337 --block-time 1 # make sure rollapp-evm not listening on same port

trash ~/.hyperlane; mkdir ~/.hyperlane; cp -r chains ~/.hyperlane/chains;

export HYP_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"

# only deploy anvil0
hyperlane core deploy

# now use hyperlane CLI to deploy only the contracts needed on anvil, making use of a foreign deployment config for dymension side

################
# FINISH HUB SETUP: 

ETH_TOKEN_CONTRACT="0x67d269191c92Caf3cD7723F116c85e6E9bf55933"
# TODO: get eth token contract from the deployment yaml

# setup the router
# TODO: require eth token contract ``
hub tx hyperlane-transfer enroll-remote-router $TOKEN_ID $ETH_DOMAIN $ETH_TOKEN_CONTRACT 0 "${HUB_FLAGS[@]}"
curl -s http://localhost:1318/hyperlane/v1/tokens/$TOKEN_ID/remote_routers # check

#################################
# DO A TRANSFER HUB -> ETHEREUM

ETH_RECIPIENT="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
ETH_RECIPIENT="0x000000000000000000000000f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
AMT=777
hub tx hyperlane-transfer dym-transfer $TOKEN_ID $ETH_DOMAIN $ETH_RECIPIENT $AMT "${HUB_FLAGS[@]}" --gas-limit 0 --max-hyperlane-fee 0adym

curl -s http://localhost:1318/hyperlane/v1/tokens/$TOKEN_ID/bridged_supply

#################################
# RELAYING
# https://docs.hyperlane.xyz/docs/guides/deploy-hyperlane-local-agents

cast wallet new
RELAYER_ADDR="0x3B3C4c9e62111E545FFc881df57ca54bC7027c7B"
RELAYER_KEY="0x29919fc136223a4f1f731d98a00c4b3b5e01f78a6314b6cc8a8b73499f057983"
cast send $RELAYER_ADDR \
--private-key $HYP_KEY \
--value $(cast tw 1)

######### scratch below

# will try to skip the validator, because using testISM
# https://docs.hyperlane.xyz/docs/guides/deploy-hyperlane-local-agents#4-run-a-relayer
# see also https://docs.hyperlane.xyz/docs/operate/relayer/run-relayer cosmos section


export CONFIG_FILES=/full/path/to/configs/agent-config-{timestamp}.json
# Pick an informative name specific to the chain you're validating
export VALIDATOR_SIGNATURES_DIR=/tmp/hyperlane-validator-signatures-<your_chain_name>

# Create the directory
mkdir -p $VALIDATOR_SIGNATURES_DIR
# Create a local tmp directory that can be accessed by docker
mkdir tmp

# Pick an informative name specific to the chain you're validating
export VALIDATOR_SIGNATURES_DIR=tmp/hyperlane-validator-signatures-<your_chain_name>

# Create the directory
mkdir -p $VALIDATOR_SIGNATURES_DIR


export CONFIG_FILES=/Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/hub_test/configs/agent-config.json # todo generalise

cargo build --release --bin relayer

./target/release/relayer \
    --db /Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/hub_test/tmp/hyperlane_db_relayer \
    --relayChains anvil0,aaadymhub \
    --allowLocalCheckpointSyncers true \
    --defaultSigner.key $RELAYER_KEY \
    --metrics-port 9091

# For tomorrow: do I have to have the  chain id in the registry be dymension_100-1?

# notes
# hub rpc = http://localhost:36657
# hub rest = http://localhost:1318


##################################################
# OPTIONAL DEBUG TIPS

# Explorer, uses https://github.com/otterscan/otterscan
docker pull otterscan/otterscan:latest
docker run -p 5100:80 \
  -e OTTERSCAN_RPC_URL="http://host.docker.internal:8545" \
   otterscan/otterscan:latest
# visit http://localhost:5100/