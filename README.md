# Driftweave

A procedural physics-visual machine. A scene is a recipe — a few dozen
numbers and a seed — so it fits in a short code and travels without a
server. The simulation runs in Rust, compiled to WebAssembly; the browser
composes the recipe and paints what falls out.

## Build

The web side has no build step. The Rust core does:

    cd core
    cargo build --release --target wasm32-unknown-unknown
    cp target/wasm32-unknown-unknown/release/driftweave_core.wasm ../js/

Then serve the folder from anywhere and open it.

On Cloudflare Pages this happens automatically via `build.sh`.
