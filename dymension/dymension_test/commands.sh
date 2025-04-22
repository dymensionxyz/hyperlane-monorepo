# Q: What is this?
# A: Some commands to run Dymension Hub + Anvil instance and connect them and relay between them
# Scenario: Dymension Hub will have collateral ADYM and Anvil will have synthetic memo

##############################################################################################
##############################################################################################
# PART 1: Start chains and deploy contracts

################
# START ANVIL: 

export HYP_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
export HYP_ADDR="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"

trash ~/.hyperlane; mkdir ~/.hyperlane; cp -r chains ~/.hyperlane/chains;

anvil --port 8545 --chain-id 31337 --block-time 1 # make sure rollapp-evm not listening on same port

# only deploy anvil0, without block explorer
hyperlane core deploy

################
# HUB: 

cd dymension/

BASE_PATH="/Users/danwt/Documents/dym/d-dymension/scripts/hyperlane_test"
source $BASE_PATH/env.sh
source /Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/dymension_test/env.sh # TODO: generic

bash scripts/setup_local.sh
dymd start --log_level=debug

hub tx hyperlane ism create-noop "${HUB_FLAGS[@]}"
sleep 5;
ISM=$(curl -s http://localhost:1318/hyperlane/v1/isms | jq '.isms.[0].id' -r); echo $ISM;

hub tx hyperlane hooks noop create "${HUB_FLAGS[@]}"
sleep 5;
NOOP_HOOK=$(curl -s http://localhost:1318/hyperlane/v1/noop_hooks | jq '.noop_hooks.[0].id' -r); echo $NOOP_HOOK;

hub tx hyperlane mailbox create $ISM $HUB_DOMAIN "${HUB_FLAGS[@]}"
sleep 5;
MAILBOX=$(curl -s http://localhost:1318/hyperlane/v1/mailboxes   | jq '.mailboxes.[0].id' -r); echo $MAILBOX;

hub tx hyperlane hooks merkle create $MAILBOX "${HUB_FLAGS[@]}"
sleep 5;
MERKLE_HOOK=$(curl -s http://localhost:1318/hyperlane/v1/merkle_tree_hooks | jq '.merkle_tree_hooks.[0].id' -r); echo $MERKLE_HOOK;

# update mailbox again. default hook (e.g. IGP), required hook (e.g. merkle tree)
hub tx hyperlane mailbox set $MAILBOX --default-hook $NOOP_HOOK --required-hook $MERKLE_HOOK "${HUB_FLAGS[@]}"

hub tx hyperlane-transfer create-collateral-token $MAILBOX $DENOM "${HUB_FLAGS[@]}" # TODO: use memo
sleep 5;
TOKEN_ID=$(curl -s http://localhost:1318/hyperlane/v1/tokens | jq '.tokens.[0].id' -r); echo $TOKEN_ID

################
# ANVIL: 

# cd hyperlane-monorepo/dymension/dymension_test

# populate addresses https://github.com/hyperlane-xyz/hyperlane-registry/blob/main/chains/kyvetestnet/addresses.yaml
touch ~/.hyperlane/chains/dymension/addresses.yaml
dasel put -f ~/.hyperlane/chains/dymension/addresses.yaml 'interchainGasPaymaster' -v $NOOP_HOOK # TODO: ok?
dasel put -f ~/.hyperlane/chains/dymension/addresses.yaml 'interchainSecurityModule' -v $ISM
dasel put -f ~/.hyperlane/chains/dymension/addresses.yaml 'mailbox' -v $MAILBOX
dasel put -f ~/.hyperlane/chains/dymension/addresses.yaml 'merkleTreeHook' -v $MERKLE_HOOK
dasel put -f ~/.hyperlane/chains/dymension/addresses.yaml 'validatorAnnounce' -v $MAILBOX
# then manually add quotes to the addresses (!!)

dasel put -f configs/warp-route-deployment.yaml 'dymension.token' -v $TOKEN_ID
dasel put -f configs/warp-route-deployment.yaml 'dymension.foreignDeployment' -v $TOKEN_ID
dasel put -f configs/warp-route-deployment.yaml 'dymension.mailbox' -v $MAILBOX
# then manually add quotes to the addresses (!!)

# now use hyperlane CLI to deploy only the contracts needed on anvil, making use of a foreign deployment config for dymension side
# it will say to deploy to dymension too, but it won't
hyperlane warp deploy

################
# FINISH HUB SETUP: 

ETH_TOKEN_CONTRACT_RAW=$(dasel -f ~/.hyperlane/deployments/warp_routes/ADYM/anvil0-config.yaml -r yaml 'tokens.index(0).addressOrDenom'); echo $ETH_TOKEN_CONTRACT_RAW;
# manual step TODO: automate
ETH_TOKEN_CONTRACT="0x0000000000000000000000004A679253410272dd5232B3Ff7cF5dbB88f295319" # Need to zero pad it! (with 0x000000000000000000000000)

hub tx hyperlane-transfer enroll-remote-router $TOKEN_ID $ETH_DOMAIN $ETH_TOKEN_CONTRACT 0 "${HUB_FLAGS[@]}" # gas = 0
sleep 5;
curl -s http://localhost:1318/hyperlane/v1/tokens/$TOKEN_ID/remote_routers # check

##############################################################################################
##############################################################################################
# PART 1: SETUP RELAYERS AND VALIDATORS
# https://docs.hyperlane.xyz/docs/guides/deploy-hyperlane-local-agents

cd rust/main
cargo build --release --bin relayer
cargo build --release --bin validator 

MONO_WORKING_DIR=/Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/dymension_test
RELAYER_DB=$MONO_WORKING_DIR/tmp/hyperlane_db_relayer
trash $MONO_WORKING_DIR/tmp/
mkdir $MONO_WORKING_DIR/tmp/

#################################
# RELAYING
# https://docs.hyperlane.xyz/docs/operate/relayer/run-relayer

cd hyperlane-monorepo/dymension/dymension_test
hyperlane registry agent-config --chains anvil0,dymension

export CONFIG_FILES=$MONO_WORKING_DIR/configs/agent-config.json
# see reference https://docs.hyperlane.xyz/docs/operate/config-reference#config_files

cd rust/main
cargo build --release --bin relayer

./target/release/relayer \
    --db $RELAYER_DB \
    --relayChains anvil0,dymension \
    --allowLocalCheckpointSyncers true \
    --defaultSigner.key $HYP_KEY \
    --metrics-port 9091 \
    --log.level debug \
    --chains.dymension.signer.type cosmosKey \
    --chains.dymension.signer.prefix dym \
    --chains.dymension.signer.key $HYP_KEY 

#################################
# DO A TRANSFER HUB -> ETHEREUM

ETH_RECIPIENT="0x000000000000000000000000f39Fd6e51aad88F6F4ce6aB8827279cffFb92266" # this is zero padded regular address
AMT=777
# TODO: use dym transfer
# hub tx hyperlane-transfer dym-transfer $TOKEN_ID $ETH_DOMAIN $ETH_RECIPIENT $AMT "${HUB_FLAGS[@]}" --max-hyperlane-fee 1000adym
hub tx hyperlane-transfer transfer $TOKEN_ID $ETH_DOMAIN $ETH_RECIPIENT $AMT "${HUB_FLAGS[@]}" --max-hyperlane-fee 1000adym --gas-limit 10000000000
sleep 5;
curl -s http://localhost:1318/hyperlane/v1/tokens/$TOKEN_ID/bridged_supply

# If relaying worked, should have some tokens here

cast call $ETH_TOKEN_CONTRACT_RAW "balanceOf(address)(uint256)" 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --rpc-url http://localhost:8545

# TODO: where am I at? I'm giving up for the day. 
# I tried with a real IGP, but with a noop validators set on the hub, and with the whitelisted relayer ISM on anvil, but it hasn't worked
# should ask about the gas ratios, also the transaction overrides (gas price) on the relayer config
# mention Other(TendermintRpcError(response error Internal error: height must be greater than 0, but got 0 (code: -32603))

##############################################################################################
##############################################################################################
# APPENDIX: DEBUGGING

# Explorer, uses https://github.com/otterscan/otterscan
docker pull otterscan/otterscan:latest
docker run -p 5100:80 \
  -e OTTERSCAN_RPC_URL="http://host.docker.internal:8545" \
   otterscan/otterscan:latest
# visit http://localhost:5100/

