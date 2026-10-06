#!/usr/bin/env bash
# Cloudflare Pages runs this on every deploy. The web side is plain files
# with no build step; the only thing that has to be compiled is the Rust
# core, and this is what compiles it.
set -euo pipefail

export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"

if ! command -v cargo >/dev/null 2>&1; then
  echo "==> installing rust"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain stable
fi
. "$CARGO_HOME/env"

echo "==> adding wasm32 target"
rustup target add wasm32-unknown-unknown

echo "==> building the core"
cargo build --manifest-path core/Cargo.toml --release --target wasm32-unknown-unknown

mkdir -p js
cp core/target/wasm32-unknown-unknown/release/driftweave_core.wasm js/driftweave.wasm

echo "==> built $(ls -la js/driftweave.wasm | awk '{print $5}') bytes"
