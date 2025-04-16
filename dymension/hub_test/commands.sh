
# scratch

https://github.com/hyperlane-xyz/hyperlane-monorepo/tree/main/typescript/cosmos-sdk

##################################################
# STEP: Chain start and setup

anvil --port 8545 --chain-id 31337 --block-time 1 # make sure rollapp-evm not listening on same port

mkdir ~/.hyperlane; cp -r chains ~/.hyperlane/chains;

export HYP_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"