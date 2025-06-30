## EXPLANATION 

### Need

# - [ ] Local hub
# - [ ] Kaspa testnet 10
# - [ ] WPRC node for kaspa testnet 10

## INSTRUCTIONS

#### PREFACE

# Recommended tabs:
# 1. dymd
# 2. wrpc node
# 3. validator
# 4. relayer 
# 5. deposit/withdraw

#### 1. Setup HUB

# clean slate
trash ~/.hyperlane; trash ~/.dymension
mkdir ~/.hyperlane; cp -r /Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/tests/kaspa_hub_test/chains ~/.hyperlane/chains

# install hub binary (dymension/)
make install
source /Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/tests/kaspa_hub_test/env.sh
scripts/setup_local.sh
dymd start --log_level=debug

export HYP_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"

# setup bridge objects on hub
CLI_VALS="0xc09dddbd26fb6dcea996ba643e8c2685c03cad57" # has (hex) key c18908a1bbe0ec588cd6522d2b02af3076a2f2c562a09bb8bf5a40f6e9a0ef1b
CLI_THRESHOLD="1"
CLI_REMOTE_ROUTER_ADDRESS="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80" # arbitrary
dymd q kas setup-bridge --validators "$CLI_VALS" --threshold "$CLI_THRESHOLD" --remote-router-address "$CLI_REMOTE_ROUTER_ADDRESS" "${HUB_FLAGS[@]}"

#### 2. START KASPA RPC NODE

# start wprc node
# https://github.com/dymensionxyz/hyperlane-monorepo/blob/ad21e8a6554999033b39949cb80c13c208bc3581/dymension/libs/kaspa/demo/multisig/README.md#L32

#### 3. SETUP VALIDATOR

# in libs/kaspa/demo/validator cargo run
# THES VALUES MUST CORRESPOND WITH agent-config.json, AND the CLI commands below
#   "validator_ism_addr": "\"0xc09dddbd26fb6dcea996ba643e8c2685c03cad5a7\"",
#   "validator_ism_priv_key": "c02e29cb65e55b3af3d8dee5d7a30504ed927436caf2e53e1e965cbd2639aced",
#   "validator_escrow_secret": "\"11013bc86d1cb199a2324130c808e90ad37d07ae8f490d063b2fb9d9aa2e898f\"",
#   "validator_escrow_pub_key": "02b1c7b586c8a0387a3c844f6a5471130bb7992346d3e906642cfd5dfce8a8129d",
#   "multisig_escrow_addr": "kaspatest:pzlq49spp66vkjjex0w7z8708f6zteqwr6swy33fmy4za866ne90v7e6pyrfr"

AGENT_TMP=/Users/danwt/Documents/dym/aaa-dym-notes/all_tasks/tasks/202505_feat_kaspa/practical/e2e/tmp
trash $AGENT_TMP/dbs
mkdir $AGENT_TMP/dbs
DB_RELAYER=$AGENT_TMP/dbs/hyperlane_db_relayer
DB_VALIDATOR=$AGENT_TMP/dbs/hyperlane_db_validator

export SIGS_VAL=$AGENT_TMP/signatures
export CONFIG_FILES=/Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/tests/kaspa_hub_test

cargo build --release --bin validator

./target/release/validator \
  --db $DB_VALIDATOR \
  --originChainName kaspatest10 \
  --reorgPeriod 1 \
  --checkpointSyncer.type localStorage \
  --checkpointSyncer.path $SIGS_VAL \
  --validator.key 0xc02e29cb65e55b3af3d8dee5d7a30504ed927436caf2e53e1e965cbd2639aced \
  --chains.dymension.signer.type cosmosKey \
  --chains.dymension.signer.prefix dym \
  --chains.dymension.signer.key $HYP_KEY \
  --metrics-port 9090 \
  --log.level info 

#### 4. SETUP RELAYER 

#### 5. SUBMIT DEPOSITS/WITHDRAWALS


#### APPENDIX: DEBUG TIPS 

curl -X POST -H "Content-Type: application/json" -d '{}' http://localhost:9090/kaspa-ping
