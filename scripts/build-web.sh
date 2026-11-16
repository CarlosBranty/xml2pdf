#!/usr/bin/env sh
# Builds the static web app into `dist/web`.
#
# Requirements: the `wasm32-unknown-unknown` Rust target and a `wasm-bindgen`
# CLI whose version matches the `wasm-bindgen` crate in Cargo.lock.
# Set WASM_BINDGEN to use a CLI that is not on the PATH.
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
out="${1:-$root/dist/web}"
wasm_bindgen="${WASM_BINDGEN:-wasm-bindgen}"

cd "$root"
cargo build --package xml2pdf-wasm --target wasm32-unknown-unknown --profile wasm-release --locked

mkdir -p "$out" "$out/fonts"
cp apps/web/index.html apps/web/styles.css apps/web/app.js apps/web/favicon.svg "$out/"
cp apps/web/fonts/* "$out/fonts/"
"$wasm_bindgen" --target web --no-typescript --remove-name-section --remove-producers-section \
    --out-dir "$out/pkg" \
    target/wasm32-unknown-unknown/wasm-release/xml2pdf_wasm.wasm

echo "Web app built in $out"
