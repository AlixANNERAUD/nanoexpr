#!/bin/sh
set -eu
cd "$(dirname "$0")"
size() {
    cargo build --quiet --profile size --lib --target wasm32-unknown-unknown --features "$1"
    wc -c < target/wasm32-unknown-unknown/size/comparison.wasm
}
kib() {
    echo "scale=1; $1 / 1024" | bc
}
baseline=$(size size)
echo "| Crate | WebAssembly size | Over baseline |"
echo "|---|---|---|"
echo "| baseline | $(kib "$baseline") KiB | |"
for library in nanoexpr meval evalexpr fasteval exmex exp-rs; do
    bytes=$(size "size,$library")
    echo "| $library | $(kib "$bytes") KiB | $(kib $((bytes - baseline))) KiB |"
done
