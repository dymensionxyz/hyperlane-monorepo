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

# TODO: I DONT THINK IGP OR GAS CONFIG IS REQUIRED FOR THIS TEST ON THE COSMOS SIDE

# update mailbox
# mailbox, default hook (e.g. IGP), required hook (e.g. merkle tree)
hub tx hyperlane mailbox set $MAILBOX --default-hook $NOOP_HOOK --required-hook $MERKLE_HOOK "${HUB_FLAGS[@]}"

DENOM="adym"
hub tx hyperlane-transfer dym-create-collateral-token $MAILBOX $DENOM "${HUB_FLAGS[@]}"
TOKEN_ID=$(curl -s http://localhost:1318/hyperlane/v1/tokens | jq '.tokens.[0].id' -r); echo $TOKEN_ID

# TODO: update the warp config with appropriate cosmos addresses

################
# ANVIL: 

anvil --port 8545 --chain-id 31337 --block-time 1 # make sure rollapp-evm not listening on same port

trash ~/.hyperlane; mkdir ~/.hyperlane; cp -r chains ~/.hyperlane/chains;

export HYP_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"

# only deploy anvil0
hyperlane core deploy

# now use hyperlane CLI to deploy only the contracts needed on anvil, making use of a foreign deployment config for dymension side
# it will say to deploy to dymension too, but it won't
hyperlane warp deploy

################
# FINISH HUB SETUP: 

ETH_TOKEN_CONTRACT=$(dasel -f ~/.hyperlane/deployments/warp_routes/ADYM/anvil0-config.yaml -r yaml 'tokens.index(0).addressOrDenom')

hub tx hyperlane-transfer enroll-remote-router $TOKEN_ID $ETH_DOMAIN $ETH_TOKEN_CONTRACT 0 "${HUB_FLAGS[@]}" # gas = 0
curl -s http://localhost:1318/hyperlane/v1/tokens/$TOKEN_ID/remote_routers # check

#################################################################################################################### 
#################################################################################################################### 
#################################################################################################################### 
#################################################################################################################### 
####################### SCRATCH STUFF BELOW


#################################
# DO A TRANSFER HUB -> ETHEREUM

ETH_RECIPIENT="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266" # without padding
ETH_RECIPIENT="0x000000000000000000000000f39Fd6e51aad88F6F4ce6aB8827279cffFb92266" # this is zero padded regular address
AMT=777
hub tx hyperlane-transfer dym-transfer $TOKEN_ID $ETH_DOMAIN $ETH_RECIPIENT $AMT "${HUB_FLAGS[@]}" --gas-limit 0 --max-hyperlane-fee 0adym

curl -s http://localhost:1318/hyperlane/v1/tokens/$TOKEN_ID/bridged_supply

#################################
# RELAYING
# https://docs.hyperlane.xyz/docs/guides/deploy-hyperlane-local-agents

cast wallet new
# manually popoulate
RELAYER_ADDR="0x95CCC68E834021347E65b404014c63c0D49ED351"
RELAYER_KEY="0x9d329776c1f8c715fef3ebf610e3f47290cb98c2bcad195e2d7429caa8cd57f1"
cast send $RELAYER_ADDR \
--private-key $HYP_KEY \
--value $(cast tw 1)

cast balance $RELAYER_ADDR

cd dymension/hub_test
THIS_BASE=$(pwd)


RELAYER_DB=$THIS_BASE/tmp/hyperlane_db_relayer
trash $RELAYER_DB

cargo build --release --bin relayer

./target/release/relayer \
    --db $RELAYER_DB \
    --relayChains anvil0,dymension \
    --allowLocalCheckpointSyncers true \
    --defaultSigner.key $RELAYER_KEY \
    --metrics-port 9091

# ONLY NECESSARY FIRST TIME, OTHERWISE USE EXISTING FILE
# see https://github.com/hyperlane-xyz/hyperlane-monorepo/blob/main/rust/main/config/testnet_config.json for examples
hyperlane registry agent-config --chains anvil0,dymension
export CONFIG_FILES=$THIS_BASE/configs/agent-config.json

##################################################
# OPTIONAL DEBUG TIPS

# Explorer, uses https://github.com/otterscan/otterscan
docker pull otterscan/otterscan:latest
docker run -p 5100:80 \
  -e OTTERSCAN_RPC_URL="http://host.docker.internal:8545" \
   otterscan/otterscan:latest
# visit http://localhost:5100/