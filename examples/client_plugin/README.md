How to build: 
```shell
cargo install --locked wasm-tools

cargo build --release --target wasm32-unknown-unknown
wasm-tools component new target/wasm32-unknown-unknown/release/client_plugin.wasm -o client_plugin.wasm
```

Then move `client_plugin.wasm` to `assets/client/plugin` in the server files
