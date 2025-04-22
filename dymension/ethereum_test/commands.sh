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


# in hyperlane-monorepo
yarn clean; yarn build; # CLEAN IS VERY IMPORTANT!
# in typescript/cli
npm uninstall -g @hyperlane-xyz/cli; yarn build; npm install -g .; hyperlane --version

##################################################
# STEP: Core contract deployment
# following hyperlane docs https://docs.hyperlane.xyz/docs/guides/deploy-warp-route
cd dymension/ethereum_test

nvm use 20

mkdir ~/.hyperlane; cp -r chains ~/.hyperlane/chains;

# this will be the first anvil private key (double check)
export HYP_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
# addr = 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266

# FIRST TIME ONLY
# it will create a deployment config 
# it should automatically detect the anvil nodes and addresses
# second time, do not need to regenerate the config file
hyperlane core init

hyperlane core deploy # if it asks for keys, double check HYP_KEY is set 
# choose testnet, and anvil 0. Do NOT verify with explorer. REPEAT WITH anvil1 (can do at the same time too)

##################################################
# STEP: deploy and verify warp routes

# FIRST TIME ONLY
# it will create a deployment config 
# do NOT use proxy contract or trusted ISM
hyperlane warp init

# NOTE: MODIFY WARP ROUTE CONFIG ACCORDINGLY
hyperlane warp deploy

##################################################
# STEP: SEND TRANSFER WITH MEMO

# First start relayer to check what happens
hyperlane relayer --chains anvil0,anvil

############
# VARIANT: Native -> Synthetic memo
# first transfer from anvil 0 to anvil 1 some tokens, to mint some synthetic erc20 on anvil 1
hyperlane warp send --relay --symbol ETH --amount 1000000
# then transfer from anvil 1 to anvil 0 using some erc20 tokens, but with a memo
cast send 0x4A679253410272dd5232B3Ff7cF5dbB88f295319 "transferRemoteMemo(uint32,bytes32,uint256,bytes)" 31337 0x0000000000000000000000004a679253410272dd5232b3ff7cf5dbb88f295319 1 0x68656c6c6f --private-key $HYP_KEY --rpc-url http://localhost:8546 --gas-limit 1000000
cast call 0x4A679253410272dd5232B3Ff7cF5dbB88f295319 "balanceOf(address)(uint256)" 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --rpc-url http://localhost:8546
############
# VARIANT: NativeMemo -> Synthetic
cast send 0x4A679253410272dd5232B3Ff7cF5dbB88f295319 "transferRemoteMemo(uint32,bytes32,uint256,bytes)" 31338 0x0000000000000000000000004a679253410272dd5232b3ff7cf5dbb88f295319 1 0x68656c6c6f --private-key $HYP_KEY --rpc-url http://localhost:8545 --gas-limit 1000000 --value 1


##################################################
# OPTIONAL DEBUG TIPS

ANV0=http://localhost:8545
ANV1=http://localhost:8546

# check eth balance
cast balance 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 

# check erc20 balance
cast call 0x4A679253410272dd5232B3Ff7cF5dbB88f295319 "balanceOf(address)(uint256)" 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --rpc-url http://localhost:8546

# Explorer, uses https://github.com/otterscan/otterscan
docker pull otterscan/otterscan:latest
docker run -p 5100:80 \
  -e OTTERSCAN_RPC_URL="http://host.docker.internal:8545" \
   otterscan/otterscan:latest
# visit http://localhost:5100/