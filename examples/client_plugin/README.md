The client uses wasm to sandbox server plugins so the build process is a little different than normal:
```shell
cargo install --locked wasm-tools

cargo build --release --target wasm32-unknown-unknown
wasm-tools component new target/wasm32-unknown-unknown/release/movement.wasm -o movement.wasm
```

Then move `movement.wasm` to `assets/client/plugin` in the server files
