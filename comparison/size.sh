#!/bin/sh
set -eu
cd "$(dirname "$0")"
echo "| Crate | WebAssembly size |"
echo "|---|---|"
for library in nanoexpr meval evalexpr fasteval exmex exp-rs; do
    cargo build --quiet --profile size --lib --target wasm32-unknown-unknown --features "size,$library"
    bytes=$(wc -c < target/wasm32-unknown-unknown/size/comparison.wasm)
    echo "| $library | $(echo "scale=1; $bytes / 1024" | bc) KiB |"
done
