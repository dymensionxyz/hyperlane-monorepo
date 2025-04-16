
# scratch

https://github.com/hyperlane-xyz/hyperlane-monorepo/tree/main/typescript/cosmos-sdk

##############################################################################################3
# STEP: Chain start and setup

################
# HUB: 

BASE_PATH="/Users/danwt/Documents/dym/d-dymension/scripts/hyperlane_test"
source $BASE_PATH/env.sh

cd dymension/
bash scripts/setup_local.sh
dymd start --log_level=debug

HUB_DOMAIN=31338
ETH_DOMAIN=31337

# create noop ism
hub tx hyperlane ism create-noop "${HUB_FLAGS[@]}"
ISM=$(curl -s http://localhost:1318/hyperlane/v1/isms | jq '.isms.[0].id' -r); echo $ISM;

# create mailbox
# ism, local domain
hub tx hyperlane mailbox create  $ISM $HUB_DOMAIN "${HUB_FLAGS[@]}"
MAILBOX=$(curl -s http://localhost:1318/hyperlane/v1/mailboxes   | jq '.mailboxes.[0].id' -r); echo $MAILBOX;

# create noop hook
hub tx hyperlane hooks noop create "${HUB_FLAGS[@]}"
NOOP_HOOK=$(curl -s http://localhost:1318/hyperlane/v1/noop_hooks | jq '.noop_hooks.[0].id' -r); echo $NOOP_HOOK;

# TODO: IGP needed? Gas config?!! (don't think so, for this test)

# update mailbox
# mailbox, default hook (e.g. IGP), required hook (e.g. merkle tree)
hub tx hyperlane mailbox set $MAILBOX --default-hook $NOOP_HOOK --required-hook $NOOP_HOOK "${HUB_FLAGS[@]}"

DENOM="adym"
hub tx hyperlane-transfer dym-create-collateral-token $MAILBOX $DENOM "${HUB_FLAGS[@]}"
TOKEN_ID=$(curl -s http://localhost:1318/hyperlane/v1/tokens | jq '.tokens.[0].id' -r); echo $TOKEN_ID

# AT THIS POINT, PUT THE TOKEN ID AS FOREIGN DEPLOYMENT IN CONFIG AND DEPLOY TO ETHEREUM 
# TODO: write ETH steps

anvil --port 8545 --chain-id 31337 --block-time 1 # make sure rollapp-evm not listening on same port

mkdir ~/.hyperlane; cp -r chains ~/.hyperlane/chains;

export HYP_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"

ETH_TOKEN_CONTRACT="0x67d269191c92Caf3cD7723F116c85e6E9bf55933"

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

