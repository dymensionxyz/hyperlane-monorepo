# Kaspa

Cheatsheet (v1.0.0)

```bash
# node
cargo run --release --bin kaspad -- --help
cargo run --release --bin kaspad -- C <config.toml>

# wallet https://github.com/kaspanet/rusty-kaspa/blob/eb71df4d284593fccd1342094c37edc8c000da85/wallet/README.md#L14
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
cargo install basic-http-server
cd wallet
cd wasm
./build-web
cd web
basic-http-server



```
