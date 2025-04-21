# Q: What is this?
# A: Some commands to run Dymension Hub + Anvil instance and connect them and relay between them
# Scenario: Dymension Hub will have collateral ADYM and Anvil will have synthetic memo

##############################################################################################3
# STEP: Start chains and deploy contracts

################
# START ANVIL: 

anvil --port 8545 --chain-id 31337 --block-time 1 # make sure rollapp-evm not listening on same port


################
# AGENT(s): 

cast wallet new

# manually popoulate
AGENT_ADDR="0x543298B2CfB5dBc7C811133cF2EB25adb5e8ACC0"
AGENT_KEY="0xde5855ae3f5782cfc8fba11cd68e85e783422bcb40d486e497e41c77edbab477"
export HYP_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
cast send $AGENT_ADDR \
--private-key $HYP_KEY \
--value $(cast tw 1)

cast balance $AGENT_ADDR

################
# HUB: 

BASE_PATH="/Users/danwt/Documents/dym/d-dymension/scripts/hyperlane_test"
source $BASE_PATH/env.sh

cd dymension/
bash scripts/setup_local.sh
dymd start --log_level=debug

HUB_DOMAIN=1260813472 
ETH_DOMAIN=31337

DENOM="adym"
hub tx hyperlane hooks igp create $DENOM "${HUB_FLAGS[@]}"
IGP=$(curl -s http://localhost:1318/hyperlane/v1/igps | jq '.igps.[0].id' -r); echo $IGP;

EXCHANGE_RATE=1 # ??
GAS_PRICE=1 # ??
GAS_OVERHEAD=200000 # ??
hub tx hyperlane hooks igp set-destination-gas-config $IGP $ETH_DOMAIN $EXCHANGE_RATE $GAS_PRICE $GAS_OVERHEAD "${HUB_FLAGS[@]}"

# '20 byte long ethereum style addresses' apparently
VALIDATOR=$AGENT_ADDR # TODO: is that right?
THRESHOLD=1
hub tx hyperlane ism create-merkle-root-multisig $VALIDATOR $THRESHOLD "${HUB_FLAGS[@]}"
ISM=$(curl -s http://localhost:1318/hyperlane/v1/isms | jq '.isms.[0].id' -r); echo $ISM;

hub tx hyperlane mailbox create $ISM $HUB_DOMAIN "${HUB_FLAGS[@]}"
MAILBOX=$(curl -s http://localhost:1318/hyperlane/v1/mailboxes   | jq '.mailboxes.[0].id' -r); echo $MAILBOX;

hub tx hyperlane hooks merkle create $MAILBOX "${HUB_FLAGS[@]}"
MERKLE_HOOK=$(curl -s http://localhost:1318/hyperlane/v1/merkle_tree_hooks | jq '.merkle_tree_hooks.[0].id' -r); echo $MERKLE_HOOK;

# update mailbox again. default hook (e.g. IGP), required hook (e.g. merkle tree)
hub tx hyperlane mailbox set $MAILBOX --default-hook $IGP --required-hook $MERKLE_HOOK "${HUB_FLAGS[@]}"

hub tx hyperlane-transfer create-collateral-token $MAILBOX $DENOM "${HUB_FLAGS[@]}" # TODO: use dym
TOKEN_ID=$(curl -s http://localhost:1318/hyperlane/v1/tokens | jq '.tokens.[0].id' -r); echo $TOKEN_ID

################
# ANVIL: 

# cd hyperlane-monorepo/dymension/dymension_test
trash ~/.hyperlane; mkdir ~/.hyperlane; cp -r chains ~/.hyperlane/chains;

# populate addresses https://github.com/hyperlane-xyz/hyperlane-registry/blob/main/chains/kyvetestnet/addresses.yaml
touch ~/.hyperlane/chains/dymension/addresses.yaml
dasel put -f ~/.hyperlane/chains/dymension/addresses.yaml 'interchainGasPaymaster' -v $IGP
dasel put -f ~/.hyperlane/chains/dymension/addresses.yaml 'interchainSecurityModule' -v $ISM
dasel put -f ~/.hyperlane/chains/dymension/addresses.yaml 'mailbox' -v $MAILBOX
dasel put -f ~/.hyperlane/chains/dymension/addresses.yaml 'merkleTreeHook' -v $MERKLE_HOOK
dasel put -f ~/.hyperlane/chains/dymension/addresses.yaml 'validatorAnnounce' -v $MAILBOX
dasel put -f ~/.hyperlane/chains/dymension/addresses.yaml 'foo' -v $MAILBOX:q
# then manually add quotes to the addresses

# TODO: update the warp config, with appropriate cosmos addresses

########### !!!!!!!!!!!!DAN!!!!!!!!!!!!! #################### THIS IS WHERE I AM ###################

# TODO: check token exchange rate
dasel put -f configs/core-config.yaml 'defaultIsm.validators.index(0)' -v $VALIDATOR

# only deploy anvil0
hyperlane core deploy

# now use hyperlane CLI to deploy only the contracts needed on anvil, making use of a foreign deployment config for dymension side
# it will say to deploy to dymension too, but it won't
# TODO: set right addresses

dasel put -f configs/warp-route-deployment.yaml 'dymension.foreignDeployment' -v $TOKEN_ID # TODO: check
dasel put -f configs/warp-route-deployment.yaml 'dymension.foreignDeployment' -v $TOKEN_ID # TODO: check

hyperlane warp deploy

################
# FINISH HUB SETUP: 

ETH_TOKEN_CONTRACT_RAW=$(dasel -f ~/.hyperlane/deployments/warp_routes/ADYM/anvil0-config.yaml -r yaml 'tokens.index(0).addressOrDenom')
ETH_TOKEN_CONTRACT="0x0000000000000000000000004A679253410272dd5232B3Ff7cF5dbB88f295319" # Need to zero pad it! (with 0x000000000000000000000000)

hub tx hyperlane-transfer enroll-remote-router $TOKEN_ID $ETH_DOMAIN $ETH_TOKEN_CONTRACT 0 "${HUB_FLAGS[@]}" # gas = 0
curl -s http://localhost:1318/hyperlane/v1/tokens/$TOKEN_ID/remote_routers # check

#################################################################################################################### 
#################################################################################################################### 
#################################################################################################################### 
#################################################################################################################### 
####################### SCRATCH STUFF BELOW

#################################
# RELAYING
# https://docs.hyperlane.xyz/docs/guides/deploy-hyperlane-local-agents
# https://docs.hyperlane.xyz/docs/operate/relayer/run-relayer



THIS_BASE=/Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/dymension_test


RELAYER_DB=$THIS_BASE/tmp/hyperlane_db_relayer
trash $RELAYER_DB

# ONLY NECESSARY FIRST TIME, OTHERWISE USE EXISTING FILE
# see https://github.com/hyperlane-xyz/hyperlane-monorepo/blob/pb/kyvetestnet-agents/rust/main/config/testnet_config.json#L2886 for an 'up to date' example
hyperlane registry agent-config --chains anvil0,dymension

export CONFIG_FILES=$THIS_BASE/configs/agent-config.json
# see reference https://docs.hyperlane.xyz/docs/operate/config-reference#config_files

cd rust/main
cargo build --release --bin relayer

trash $RELAYER_DB
./target/release/relayer \
    --db $RELAYER_DB \
    --relayChains anvil0,dymension \
    --allowLocalCheckpointSyncers true \
    --defaultSigner.key $AGENT_KEY \
    --metrics-port 9091 \
    --log.level debug \
    --chains.dymension.signer.type cosmosKey \
    --chains.dymension.signer.prefix dym \
    --chains.dymension.signer.key $AGENT_KEY 

#################################
# DO A TRANSFER HUB -> ETHEREUM

ETH_RECIPIENT="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266" # without padding
ETH_RECIPIENT="0x000000000000000000000000f39Fd6e51aad88F6F4ce6aB8827279cffFb92266" # this is zero padded regular address
AMT=777
hub tx hyperlane-transfer dym-transfer $TOKEN_ID $ETH_DOMAIN $ETH_RECIPIENT $AMT "${HUB_FLAGS[@]}" --max-hyperlane-fee 0adym

curl -s http://localhost:1318/hyperlane/v1/tokens/$TOKEN_ID/bridged_supply

# If relaying worked, should have some tokens here
cast call 0x4A679253410272dd5232B3Ff7cF5dbB88f295319 "balanceOf(address)(uint256)" 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --rpc-url http://localhost:8545

##################################################
# OPTIONAL DEBUG TIPS

# Explorer, uses https://github.com/otterscan/otterscan
docker pull otterscan/otterscan:latest
docker run -p 5100:80 \
  -e OTTERSCAN_RPC_URL="http://host.docker.internal:8545" \
   otterscan/otterscan:latest
# visit http://localhost:5100/