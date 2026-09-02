# Nommetric

Chromium extension that overlays your own scores and notes onto Deliveroo menu items.
Rust → WASM, no npm.

## Prerequisites

```sh
rustup target add wasm32-unknown-unknown
cargo binstall wasm-pack
```

## Build and load

```sh
./build.sh [--release]
cargo test
```

Then in a Chromium-based browser, turn on Developer mode, and load the extension from the `extension` folder.

## Dev loop

```sh
cargo watch -s ./build.sh
```
