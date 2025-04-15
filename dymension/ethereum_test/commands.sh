###########################
# Prelims

# The foundry toolchain: https://book.getfoundry.sh/getting-started/installation
curl -L https://foundry.paradigm.xyz | bash
foundryup
# it installs forge, cast, anvil, and chisel for units tests, sending commands, and running local nodes

# node
# recommended to use nvm to install node v20
# https://github.com/nvm-sh/nvm
nvm install 20
nvm use 20

# OPTIONALLY:
trash ~/.hyperlane # for a fresh start

#########################################################################################
#########################################################################################
# Q: WHAT IS THIS?
# A: It's not a script, but rather some commands, which should be copy pasted as appropriate per the instructions, while in the right directories.
#########################################################################################

##################################################
# STEP: Local ethereum nodes setup

anvil --port 8545 --chain-id 31337 --block-time 1 # make sure rollapp-evm not listening on same port
anvil --port 8546 --chain-id 31338 --block-time 1

# one node is 
# http://localhost:8545
# 31337
# another node is
# http://localhost:8546
# 31338

# only necessary first time
hyperlane registry init

##################################################
# STEP: Build CLI with our changes
# This can be finicky. Make sure that typescript/sdk is successfully building first, and only then build typescript/cli
# Once each is building, it's possible to do yarn build from typescript/
# Use yarn clean to make sure nothing weird happens.
# Note: it's NOT necessary to change the dependency path in typescript/cli/package.json to point to the local path of sdk

# commands:
yarn build
yarn clean
yarn version:update;
npm uninstall -g @hyperlane-xyz/cli; 
npm install -g .;
hyperlane --version

##################################################
# STEP: Core contract deployment
# following hyperlane docs https://docs.hyperlane.xyz/docs/guides/deploy-warp-route
cd dymension/ethereum_test

nvm use 20

mkdir ~/.hyperlane
cp -r chains ~/.hyperlane/chains

# this will be the first anvil private key (double check)
export HYP_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
# addr = 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266

# FIRST TIME ONLY
# it will create a deployment config 
# it should automatically detect the anvil nodes and addresses
# second time, do not need to regenerate the config file
hyperlane core init

hyperlane core deploy # if it asks for keys, double check HYP_KEY is set 
# choose testnet, and anvil 0. Do NOT verify with explorer. REPEAT WITH anvil1

##################################################
# STEP: deploy and verify warp routes

# FIRST TIME ONLY
# it will create a deployment config 
# do NOT use proxy contract or trusted ISM
hyperlane warp init
#    anvil0:
#      isNft: false
#      type: nativeMemo
#      owner: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
#    anvil1:
#      isNft: false
#      type: synthetic
#      owner: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"

hyperlane warp deploy

# check that it worked
hyperlane warp send --relay --symbol ETH

##################################################
# STEP: SEND TRANSFER WITH MEMO

CONTRACT_ADDR=$(dasel -f ~/.hyperlane/deployments/warp_routes/ETH/anvil0-anvil1-config.yaml -r yaml '.tokens.index(0).addressOrDenom')
EXAMPLE_MEMO="0x0a85010a087472616e7366657212096368616e6e656c2d301a0a0a046172617812023530222a64796d317133303476717239677870766c366b766c656b747238637867743532747879636138347333782a2a64796d317965637672677a37797032366b65617861347230303535347575676174786665676b3736687a320038f0e5dfb9a5e8b49918122c0a2a64796d317965637672677a37797032366b65617861347230303535347575676174786665676b3736687a"

# initiate the transer with memo
cast send $CONTRACT_ADDR "setMemoForNextTransfer(bytes)" "$EXAMPLE_MEMO" --private-key "$HYP_KEY" --rpc-url http://localhost:8545 --gas-limit 1000000

# relay the transfer, this will show the message and message body in the logs
hyperlane warp send --relay --symbol ETH --amount 1000000

# manually put message here
OUT_MESSAGE="0x030000000100007a690000000000000000000000004a679253410272dd5232b3ff7\ cf5dbb88f29531900007a6a0000000000000000000000004a679253410272dd5232b3ff7cf5db\ b88f295319000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266000\ 00000000000000000000000000000000000000000000000000000000000010a85010a08747261\ 6e7366657212096368616e6e656c2d301a0a0a046172617812023530222a64796d31713330347\ 6717239677870766c366b766c656b747238637867743532747879636138347333782a2a64796d\ 317965637672677a37797032366b65617861347230303535347575676174786665676b3736687\ a320038f0e5dfb9a5e8b49918122c0a2a64796d317965637672677a37797032366b6561786134\ 7230303535347575676174786665676b3736687a"
# check that the memo is interpreted correctly by the hub
dymd q forward hl-decode message $OUT_MESSAGE # it should show the ibc packet

##########################
# WIP: USE CAST
cast send 0x4A679253410272dd5232B3Ff7cF5dbB88f295319 "transferRemoteMemo(uint32,bytes32,uint256,bytes)" 31337 0x0000000000000000000000004a679253410272dd5232b3ff7cf5dbb88f295319 1 0x68656c6c6f --private-key $HYP_KEY --rpc-url http://localhost:8546 --gas-limit 1000000 --value 1
cast send 0x4A679253410272dd5232B3Ff7cF5dbB88f295319 "transferRemote(uint32,bytes32,uint256)" 31337 0x0000000000000000000000004a679253410272dd5232b3ff7cf5dbb88f295319 1 --private-key $HYP_KEY --rpc-url http://localhost:8546 --gas-limit 1000000

##################################################
# OPTIONAL DEBUG TIPS

ANV0=http://localhost:8545
ANV1=http://localhost:8546

cast balance 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --rpc-url $ANV0

# Explorer, uses https://github.com/otterscan/otterscan
# GUI is at http://localhost:5100/

ANVIL_RPC_URL=http://127.0.0.1:8545

docker run \
  --rm \
  -p 5100:80 \
  --name otterscan \
  -d \
  --env OTTERSCAN_CONFIG='{
    "erigonURL": "'$ANVIL_RPC_URL'",
    "assetsURLPrefix": "http://127.0.0.1:5175",
    "branding": {
        "siteName": "My Otterscan",
        "networkTitle": "Dev Network"
    },
}' \
otterscan/otterscan:latest
