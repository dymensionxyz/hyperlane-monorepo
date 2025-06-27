## NOTES

- [ ] Make a validator escrow

## INSTRUCTIONS

trash ~/.hyperlane; trash ~/.dymension

mkdir ~/.hyperlane; cp -r /Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/tests/kaspa_hub_test/chains ~/.hyperlane/chains

# install hub binary
source /Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/tests/kaspa_hub_test/env.sh
dymd start --log_level=debug

export HYP_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"

hub tx hyperlane ism create-noop "${HUB_FLAGS[@]}"
sleep 7;
ISM=$(curl -s http://localhost:1318/hyperlane/v1/isms | jq '.isms.[0].id' -r); echo $ISM;

hub tx hyperlane hooks noop create "${HUB_FLAGS[@]}"
sleep 7;
NOOP_HOOK=$(curl -s http://localhost:1318/hyperlane/v1/noop_hooks | jq '.noop_hooks.[0].id' -r); echo $NOOP_HOOK;

hub tx hyperlane mailbox create $ISM $HUB_DOMAIN "${HUB_FLAGS[@]}"
sleep 7;
MAILBOX=$(curl -s http://localhost:1318/hyperlane/v1/mailboxes   | jq '.mailboxes.[0].id' -r); echo $MAILBOX;

hub tx hyperlane hooks merkle create $MAILBOX "${HUB_FLAGS[@]}"
sleep 7;
MERKLE_HOOK=$(curl -s http://localhost:1318/hyperlane/v1/merkle_tree_hooks | jq '.merkle_tree_hooks.[0].id' -r); echo $MERKLE_HOOK;

# update mailbox again. default hook (e.g. IGP), required hook (e.g. merkle tree)
hub tx hyperlane mailbox set $MAILBOX --default-hook $NOOP_HOOK --required-hook $MERKLE_HOOK "${HUB_FLAGS[@]}"

hub tx hyperlane-transfer create-synthetic-token $MAILBOX $DENOM "${HUB_FLAGS[@]}"
sleep 7;
TOKEN_ID=$(curl -s http://localhost:1318/hyperlane/v1/tokens | jq '.tokens.[0].id' -r); echo $TOKEN_ID

hub tx hyperlane-transfer enroll-remote-router $TOKEN_ID $ETH_DOMAIN $ETH_TOKEN_CONTRACT 0 "${HUB_FLAGS[@]}" # gas = 0
sleep 7;
curl -s http://localhost:1318/hyperlane/v1/tokens/$TOKEN_ID/remote_routers # check

# needed?
touch ~/.hyperlane/chains/dymension/tests/addresses.yaml
dasel put -f ~/.hyperlane/chains/dymension/tests/addresses.yaml 'interchainGasPaymaster' -v $NOOP_HOOK
dasel put -f ~/.hyperlane/chains/dymension/tests/addresses.yaml 'interchainSecurityModule' -v $ISM
dasel put -f ~/.hyperlane/chains/dymension/tests/addresses.yaml 'mailbox' -v $MAILBOX
dasel put -f ~/.hyperlane/chains/dymension/tests/addresses.yaml 'merkleTreeHook' -v $MERKLE_HOOK
dasel put -f ~/.hyperlane/chains/dymension/tests/addresses.yaml 'validatorAnnounce' -v $MAILBOX

dasel put -f configs/warp-route-deployment.yaml 'dymension.token' -v $TOKEN_ID
dasel put -f configs/warp-route-deployment.yaml 'dymension.foreignDeployment' -v $TOKEN_ID
dasel put -f configs/warp-route-deployment.yaml 'dymension.mailbox' -v $MAILBOX