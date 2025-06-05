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
Faucet: https://faucet-tn10.kaspanet.io/

### Node

<!-- cargo run --release --bin kaspad -- --testnet --netsuffix=10 --utxoindex -->

cargo run --release --bin kaspad -- -C /Users/danwt/Documents/dym/d-hyperlane-monorepo/dymension/libs/kaspa/demo-multisig/kaspad.toml

## Multisig Theory

### Src

- Sig definitions https://github.com/kaspanet/rusty-kaspa/blob/eb71df4d284593fccd1342094c37edc8c000da85/crypto/txscript/src/lib.rs#L55-L65
- PSKT Multisig examples https://github.com/kaspanet/rusty-kaspa/blob/eb71df4d284593fccd1342094c37edc8c000da85/wallet/pskt/examples/multisig.rs#L12

### Appendix

Learned on discord:

"Segwit (Bitcoin https://learnmeabitcoin.com/technical/upgrades/segregated-witness/) is not in Kaspa because Kaspa TX ID doesn't include script signature (https://github.com/kaspanet/rusty-kaspa/blob/eaadfa6230fc376f314d9a504c4c70fbc0416844/consensus/core/src/hashing/tx.rs#L20)"

"Multisig upper bound is N=20"

"There are 3 types of addresses currently: schnorr, ecdsa, script. So natively there's no support of multisig. Multisig is only implemented as p2sh. Probably Frost threshold scheme can be used to work with a single schnorr signature and multiple parties, but it's a completely different story"
