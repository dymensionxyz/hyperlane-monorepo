## NOTES

- [ ] Launch validator and relayer
- [ ] Create ISM

## INSTRUCTIONS

trash ~/.hyperlane; trash ~/.dymension

mkdir ~/.hyperlane; cp -r /Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/tests/kaspa_hub_test/chains ~/.hyperlane/chains

# install hub binary
source /Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/tests/kaspa_hub_test/env.sh
scripts/setup_local.sh
dymd start --log_level=debug

export HYP_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"

CLI_VALS="0xc09dddbd26fb6dcea996ba643e8c2685c03cad57" # has (hex) key c18908a1bbe0ec588cd6522d2b02af3076a2f2c562a09bb8bf5a40f6e9a0ef1b
CLI_THRESHOLD="1"
CLI_REMOTE_ROUTER_ADDRESS="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80" # arbitrary
dymd q kas setup-bridge --validators "$CLI_VALS" --threshold "$CLI_THRESHOLD" --remote-router-address "$CLI_REMOTE_ROUTER_ADDRESS" "${HUB_FLAGS[@]}"

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

dymd tx kas bootstrap \
  --mailbox "0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B" \
  --ism "0x1234567890123456789012345678901234567890" \
  --outpoint '{"transaction_id": "EiIzRFVmd4iZqrvM3e7/ABEjM0RWZ3iJmqu8zd7v/AA=", "index": 0}' \
  --from my-validator-key \
  --chain-id dymension_1100-1 \
  -y


##### TODO: scratch below

## start wprc node
# https://github.com/dymensionxyz/hyperlane-monorepo/blob/ad21e8a6554999033b39949cb80c13c208bc3581/dymension/libs/kaspa/demo/multisig/README.md#L32

## Running validator

AGENT_TMP=/Users/danwt/Documents/dym/aaa-dym-notes/all_tasks/tasks/202505_feat_kaspa/practical/e2e/tmp
trash $AGENT_TMP/dbs
mkdir $AGENT_TMP/dbs
DB_RELAYER=$AGENT_TMP/dbs/hyperlane_db_relayer
DB_VALIDATOR=$AGENT_TMP/dbs/hyperlane_db_validator
# DB_VALIDATOR_2=$AGENT_TMP/dbs/hyperlane_db_validator_2

export VALIDATOR_SIGNATURES_DIR=$AGENT_TMP/signatures # official name

export SIGS_VAL=$AGENT_TMP/signatures
export CONFIG_FILES=/Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/docs/kaspa/relayer/example/config/agent-config.json

# set AWS environment variables
# export AWS_ACCESS_KEY_ID=ABCDEFGHIJKLMNOP
# export AWS_SECRET_ACCESS_KEY=xX-haha-nice-try-Xx
# {
#   "validator_ism_addr": "\"0xc09dddbd26fb6dcea996ba643e8c2685c03cad57\"",
#   "validator_ism_priv_key": "c02e29cb65e55b3af3d8dee5d7a30504ed927436caf2e53e1e965cbd2639aced",
#   "validator_escrow_secret": "\"11013bc86d1cb199a2324130c808e90ad37d07ae8f490d063b2fb9d9aa2e898f\"",
#   "validator_escrow_pub_key": "02b1c7b586c8a0387a3c844f6a5471130bb7992346d3e906642cfd5dfce8a8129d",
#   "multisig_escrow_addr": "kaspatest:pzlq49spp66vkjjex0w7z8708f6zteqwr6swy33fmy4za866ne90v7e6pyrfr"
# }

cargo build --release --bin validator

# cargo run --release --bin validator -- \
./target/release/validator \
  --db $DB_VALIDATOR \
  --originChainName kaspatest10 \
  --reorgPeriod 1 \
  --checkpointSyncer.type localStorage \
  --checkpointSyncer.path $SIGS_VAL \
  --validator.key 0xc02e29cb65e55b3af3d8dee5d7a30504ed927436caf2e53e1e965cbd2639aced \
  --metrics-port 9090 \
  --log.level debug  \
  --chains.dymension.signer.type cosmosKey \
  --chains.dymension.signer.prefix dym \
  --chains.dymension.signer.key $HYP_KEY \

#  ./target/release/relayer \
 cargo run --release --bin relayer -- \
    --db $DB_RELAYER \
    --relayChains anvil0,dymension \
    --allowLocalCheckpointSyncers true \
    --defaultSigner.key $HYP_KEY \
    --metrics-port 9091 \
    --chains.dymension.signer.type cosmosKey \
    --chains.dymension.signer.prefix dym \
    --chains.dymension.signer.key $HYP_KEY \
    --log.level debug 


# gpt etc below
# run the Validator
./target/release/validator \
  --db $DB_VALIDATOR \
  --originChainName dymension \
  --reorgPeriod 1 \
  --validator.region us-east-1 \
  --checkpointSyncer.region us-east-1 \
  --validator.type aws \
  --chains.<your_chain_name>.signer.type aws \
  --chains.<your_chain_name>.signer.region<region_name> \
  --validator.id alias/hyperlane-validator-signer-<your_chain_name> \
  --chains.<your_chain_name>.signer.id alias/hyperlane-validator-signer-<your_chain_name> \
  --checkpointSyncer.type s3 \
  --checkpointSyncer.bucket hyperlane-validator-signatures-<your_chain_name>\