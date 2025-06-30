## NOTES

- [ ] Launch validator and relayer
- [ ] Create ISM

## INSTRUCTIONS

trash ~/.hyperlane; trash ~/.dymension

mkdir ~/.hyperlane; cp -r /Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/tests/kaspa_hub_test/chains ~/.hyperlane/chains

# install hub binary
source /Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/tests/kaspa_hub_test/env.sh
dymd start --log_level=debug

export HYP_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"

CLI_VALS="0x9695e09597f3111b183700e06d6f1a7d50ea1aee" # has (hex) key c18908a1bbe0ec588cd6522d2b02af3076a2f2c562a09bb8bf5a40f6e9a0ef1b
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

cargo run --release --bin kaspad -- -C /Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/libs/kaspa/demo-multisig/kaspad.toml

## Running validator

AGENT_TMP=/Users/danwt/Documents/dym/aaa-dym-notes/all_tasks/tasks/202505_feat_kaspa/practical/e2e/tmp
trash $AGENT_TMP/dbs
mkdir $AGENT_TMP/dbs
DB_RELAYER=$AGENT_TMP/dbs/hyperlane_db_relayer
DB_VALIDATOR_1=$AGENT_TMP/dbs/hyperlane_db_validator_1
DB_VALIDATOR_2=$AGENT_TMP/dbs/hyperlane_db_validator_2

export VALIDATOR_SIGNATURES_DIR=$AGENT_TMP/signatures

export CONFIG_FILES=/Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/docs/kaspa/relayer/example/config/agent-config.json

# set AWS environment variables
# export AWS_ACCESS_KEY_ID=ABCDEFGHIJKLMNOP
# export AWS_SECRET_ACCESS_KEY=xX-haha-nice-try-Xx

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

 ./target/release/relayer \
    --db $DB_RELAYER \
    --relayChains anvil0,dymension \
    --allowLocalCheckpointSyncers true \
    --defaultSigner.key $HYP_KEY \
    --metrics-port 9091 \
    --chains.dymension.signer.type cosmosKey \
    --chains.dymension.signer.prefix dym \
    --chains.dymension.signer.key $HYP_KEY \
    --log.level debug 


# gpt below
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