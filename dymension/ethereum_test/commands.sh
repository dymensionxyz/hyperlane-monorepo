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

#########################################################################################
#########################################################################################
# Q: WHAT IS THIS?
# A: It's not a script, but rather some commands, which should be copy pasted as appropriate per the instructions, while in the right directories.
#########################################################################################



##################################################
# Local ethereum nodes setup

anvil --port 8545 --chain-id 31337 --block-time 1
anvil --port 8546 --chain-id 31338 --block-time 1

# one node is 
# http://localhost:8545
# 31337
# another node is
# http://localhost:8546
# 31338


##################################################
# Core contract deployment
# following hyperlane docs https://docs.hyperlane.xyz/docs/guides/deploy-warp-route

nvm use 20

# this will be the first anvil private key (double check)
export HYP_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
# addr = 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266

# in tasks/..hyperlane-local-test/
hyperlane core init
# it will create a deployment config 
hyperlane core deploy # if it asks for keys, double check HYP_KEY is set 

##################################################
# Rebuild CLI to get custom changes

cd typescript/cli

yarn version:update; yarn build;
npm uninstall -g @hyperlane-xyz/cli; npm install -g .;
hyperlane --version

##################################################
# Warp routes

hyperlane warp init
#    anvil0:
#      isNft: false
#      type: nativeMemo
#      owner: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
#    anvil1:
#      isNft: false
#      type: syntheticMemo
#      owner: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
hyperlane warp deploy

# it worked..
# to demo and check setup
hyperlane warp send --relay --symbol ETH

##################################################
# MEMO CHECK 

EXAMPLE_MEMO="0x0a85010a087472616e7366657212096368616e6e656c2d301a0a0a046172617812023530222a64796d317133303476717239677870766c366b766c656b747238637867743532747879636138347333782a2a64796d317965637672677a37797032366b65617861347230303535347575676174786665676b3736687a320038f0e5dfb9a5e8b49918122c0a2a64796d317965637672677a37797032366b65617861347230303535347575676174786665676b3736687a"
cast send $CONTRACT_ADDR "setMemoForNextTransfer(bytes)" "$EXAMPLE_MEMO" --private-key "$HYP_KEY" --rpc-url http://localhost:8545 --gas-limit 1000000

hyperlane warp send --relay --symbol ETH

OUT_MESSAGE="0x030000000400007a690000000000000000000000004a679253410272dd5232b3ff7\ cf5dbb88f29531900007a6a0000000000000000000000004a679253410272dd5232b3ff7cf5db\ b88f295319000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266000\ 00000000000000000000000000000000000000000000000000000000000010a85010a08747261\ 6e7366657212096368616e6e656c2d301a0a0a046172617812023530222a64796d31713330347\ 6717239677870766c366b766c656b747238637867743532747879636138347333782a2a64796d\ 317965637672677a37797032366b65617861347230303535347575676174786665676b3736687\ a320038f0e5dfb9a5e8b49918122c0a2a64796d317965637672677a37797032366b6561786134\ 7230303535347575676174786665676b3736687a"
dymd q forward hyperlane-decode message $OUT_MESSAGE
# it should show the ibc packet

##################################################
# Debugging
ANV0=http://localhost:8545
ANV1=http://localhost:8546

##################################################
# Other useful things
cast balance 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 --rpc-url http://localhost:8545
