## Instructions

### Tools

```bash
rustup update

cargo version
# cargo 1.87.0 (99624be96 2025-05-06)
rustc -V
# rustc 1.87.0 (17067e9ac 2025-05-09)

# Tested with https://github.com/kaspanet/rusty-kaspa v1.0.0 (Crescendo)
```

### Resources

TN10 is running v1.0.0 https://wiki.kaspa.org/en/testnets
Endpoint: https://api-tn10.kaspa.org/
API: https://api.kaspa.org/docs
Faucet: https://faucet-tn10.kaspanet.io/

### Node

<!-- cargo run --release --bin kaspad -- --testnet --netsuffix=10 --utxoindex -->

cargo run --release --bin kaspad -- -C /Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/libs/kaspa/demo-multisig/kaspad.toml

## Multisig Theory
