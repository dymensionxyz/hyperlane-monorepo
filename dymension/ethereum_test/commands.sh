# following h
# and https://docs.hyperlane.xyz/docs/guides/deploy-warp-route

nvm use 20

export HYP_KEY=??


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

# Accounts and keys should be the same on both anvil instances
# Available Accounts
# ==================
# 
# (0) 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 (10000.000000000000000000 ETH)
# (1) 0x70997970C51812dc3A010C7d01b50e0d17dc79C8 (10000.000000000000000000 ETH)
# (2) 0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC (10000.000000000000000000 ETH)
# (3) 0x90F79bf6EB2c4f870365E785982E1f101E93b906 (10000.000000000000000000 ETH)
# (4) 0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65 (10000.000000000000000000 ETH)
# (5) 0x9965507D1a55bcC2695C58ba16FB37d819B0A4dc (10000.000000000000000000 ETH)
# (6) 0x976EA74026E726554dB657fA54763abd0C3a0aa9 (10000.000000000000000000 ETH)
# (7) 0x14dC79964da2C08b23698B3D3cc7Ca32193d9955 (10000.000000000000000000 ETH)
# (8) 0x23618e81E3f5cdF7f54C3d65f7FBc0aBf5B21E8f (10000.000000000000000000 ETH)
# (9) 0xa0Ee7A142d267C1f36714E4a8F75612F20a79720 (10000.000000000000000000 ETH)
# 
# Private Keys
# ==================
# 
# (0) 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
# (1) 0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d
# (2) 0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a
# (3) 0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6
# (4) 0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a
# (5) 0x8b3a350cf5c34c9194ca85829a2df0ec3153be0318b5e2d3348e872092edffba
# (6) 0x92db14e403b83dfe3df233f83dfa3a0d7096f21ca9b0d6d6b8d88b2b4ec1564e
# (7) 0x4bbbf85ce3377467afe5d46f804f221813b2bb87f24d81f60f1fcdbf7cbf4356
# (8) 0xdbda1821b80551c9d65939329250298aa3472ba22feea921c0cf5d620ea67b97
# (9) 0x2a871d0798f97d79848a013d4936a73bf4cc922c825d33c1cf7073dff6d409c6

# Use the private key from anvil0 as the HYP KEY
# Note that the hyperlane CLI also hardcodes this for all the anvil commands
export HYP_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
# addr = 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266


##################################################
# Core contract deployment

# in tasks/..hyperlane-local-test/
hyperlane core init
# it will create a deployment config 
hyperlane core deploy # make sure HYP_KEY is set first

##################################################
# Rebuild CLI

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
