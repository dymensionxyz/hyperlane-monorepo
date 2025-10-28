# How to be a Kaspa bridge validator

## Key Generation

In hyperlane-monorepo/dymension/libs/kaspa/demo/user do `cargo run validator`.

It outputs something like

```
[
  {
    "validator_ism_addr": "0x2541ca4d67d89897d51c2bf25b1fb602eca4ae5c",
    "validator_ism_priv_key": "92940b5c00eb0e8c62f4c0d344b4fee4064c3ac51297159bf77874744e47e016",
    "validator_escrow_secret": "\"b55335e614dacb747ee4bfb5bd95e9cdb7291d32542b27924f06cb1299a2cc5a\"",
    "validator_escrow_pub_key": "0200b77b8e8f871121cda5a5c98938c7057ddee9aed930eea0dbb86dd23cbfd300",
    "multisig_escrow_addr": null
  }
]
```

Give Dymension team validator_ism_addr and validator_escrow_pub_key. Don't worry about multisig_escrow_addr. Backup the private keys.

## Config

Use the agent-config.json template provided by Dymension team. Populate .chains.<kaspa>.validatorEscrowPrivateKey with the escrow secret validator_escrow_secret (keep quotes). Also populate .valiator.key with validator_ism_priv_key. Check agent-config.example.json for an informational example.

## Running

Copy the dummy kaspa.mainnet.wallet to ~/.kaspa/kaspa.wallet: `cp <dummy> ~/.kaspa/kaspa.wallet. This wallet is just to stop the Kaspa client crashing. Signing uses the validator_escrow_secret generated before.

Make a database directory in place of your choosing

```
DB_VALIDATOR=<your directory>
```

### Setup Environment Variables

```bash
CONFIG_FILES=<path to populated agent-config.json>
DB_VALIDATOR=<your database directory>
ORIGIN_CHAIN=kaspatest10  # or mainnet

# Save to bash profile
echo 'export CONFIG_FILES='${CONFIG_FILES} > $HOME/.bash_profile
echo 'export DB_VALIDATOR='${DB_VALIDATOR} >> $HOME/.bash_profile

cat <<'EOF' >> $HOME/.bash_profile
echo -e "\n\033[0;93mSTATUS:\n======\n"
echo -n "TMUX: "; tmux ls
echo
echo "CONFIG_FILES: ${CONFIG_FILES}"
echo "DB_VALIDATOR: ${DB_VALIDATOR}"
echo
echo -e "\033[0m"
source "$HOME/.cargo/env"
EOF

source ~/.bash_profile

# Build the validator
cd ${HOME}/hyperlane-monorepo/rust/main
cargo build --release --bin validator
```

### Option 1: Run with systemd (recommended)

```bash
# Create systemd service
sudo tee <<EOF >/dev/null /etc/systemd/system/validator.service
[Unit]
Description=Kaspa Bridge Validator
After=network-online.target
[Service]
WorkingDirectory=${HOME}/hyperlane-monorepo/rust/main
User=$USER
Environment="CONFIG_FILES=${CONFIG_FILES}"
ExecStart=${HOME}/hyperlane-monorepo/rust/main/target/release/validator \
--db ${DB_VALIDATOR} \
--originChainName ${ORIGIN_CHAIN} \
--reorgPeriod 1 \
--checkpointSyncer.type localStorage \
--checkpointSyncer.path ARBITRARY_VALUE_FOOBAR \
--metrics-port 9090 \
--log.level info
Restart=on-failure
RestartSec=10
LimitNOFILE=65535
[Install]
WantedBy=multi-user.target
EOF

# Reload systemd and start the service
sudo systemctl daemon-reload
sudo systemctl enable validator
sudo systemctl start validator

# View logs
journalctl -u validator -f -o cat
```

### Option 2: Run with tmux

```bash
tmux
echo $DB_VALIDATOR && echo $CONFIG_FILES && sleep 3s
cd ${HOME}/hyperlane-monorepo/rust/main
./target/release/validator \
--db $DB_VALIDATOR \
--originChainName $ORIGIN_CHAIN \
--reorgPeriod 1 \
--checkpointSyncer.type localStorage \
--checkpointSyncer.path ARBITRARY_VALUE_FOOBAR \
--metrics-port 9090 \
--log.level info
```

### Managing the systemd Service

```bash
# Check status
sudo systemctl status validator

# Restart
sudo systemctl restart validator

# Stop
sudo systemctl stop validator

# Disable autostart
sudo systemctl disable validator

# View logs
journalctl -u validator -f -o cat
```

## Exposure

Make sure 9090 or whatever chosen metrics-port is exposed and tell Dymension team. Your validator will answer queries at that port.
