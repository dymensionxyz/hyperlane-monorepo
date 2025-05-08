trash ~/.hyperlane; trash ~/.dymension

mkdir ~/.hyperlane; cp -r /Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/ethereum_test/chains ~/.hyperlane/chains

export HYP_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"

cp /Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/ethereum_test/configs/core-config.yaml configs

hyperlane core deploy

# run steps from ethereum_test/commands.sh, up to but not including token

hub tx hyperlane-transfer dym-create-synthetic-token $MAILBOX "${HUB_FLAGS[@]}"
sleep 7;
TOKEN_ID=$(curl -s http://localhost:1318/hyperlane/v1/tokens | jq '.tokens.[0].id' -r); echo $TOKEN_ID

touch ~/.hyperlane/chains/dymension/addresses.yaml
dasel put -f ~/.hyperlane/chains/dymension/addresses.yaml 'interchainGasPaymaster' -v $NOOP_HOOK
dasel put -f ~/.hyperlane/chains/dymension/addresses.yaml 'interchainSecurityModule' -v $ISM
dasel put -f ~/.hyperlane/chains/dymension/addresses.yaml 'mailbox' -v $MAILBOX
dasel put -f ~/.hyperlane/chains/dymension/addresses.yaml 'merkleTreeHook' -v $MERKLE_HOOK
dasel put -f ~/.hyperlane/chains/dymension/addresses.yaml 'validatorAnnounce' -v $MAILBOX

cp /Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/ethereum_test/configs/warp-route-deployment.yaml configs

dasel put -f configs/warp-route-deployment.yaml 'dymension.token' -v $TOKEN_ID
dasel put -f configs/warp-route-deployment.yaml 'dymension.foreignDeployment' -v $TOKEN_ID
dasel put -f configs/warp-route-deployment.yaml 'dymension.mailbox' -v $MAILBOX





