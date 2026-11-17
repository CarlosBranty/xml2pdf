#!/usr/bin/env sh
# Builds the static web app into `dist/web`.
#
# Requirements: the `wasm32-unknown-unknown` Rust target and a `wasm-bindgen`
# CLI whose version matches the `wasm-bindgen` crate in Cargo.lock.
# Set WASM_BINDGEN to use a CLI that is not on the PATH.
# Set SITE_URL (the public address, without a trailing slash) to publish the canonical
# address, the social preview image and the sitemap.
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
out="${1:-$root/dist/web}"
wasm_bindgen="${WASM_BINDGEN:-wasm-bindgen}"

cd "$root"
cargo build --package xml2pdf-wasm --target wasm32-unknown-unknown --profile wasm-release --locked

mkdir -p "$out" "$out/fonts"
cp apps/web/styles.css apps/web/app.js apps/web/platform.js apps/web/favicon.svg apps/web/og.png "$out/"

# Pages that mention the public address take it from SITE_URL. Without it,
# the lines that need an absolute address are left out.
site_url="${SITE_URL:-}"
site_url="${site_url%/}"
for page in index.html robots.txt; do
    if [ -n "$site_url" ]; then
        sed "s|__SITE_URL__|$site_url|g" "apps/web/$page" > "$out/$page"
    else
        sed '/__SITE_URL__/d' "apps/web/$page" > "$out/$page"
    fi
done
if [ -n "$site_url" ]; then
    printf '<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n  <url><loc>%s/</loc></url>\n</urlset>\n' "$site_url" > "$out/sitemap.xml"
else
    rm -f "$out/sitemap.xml"
fi
cp apps/web/fonts/* "$out/fonts/"
"$wasm_bindgen" --target web --no-typescript --remove-name-section --remove-producers-section \
    --out-dir "$out/pkg" \
    target/wasm32-unknown-unknown/wasm-release/xml2pdf_wasm.wasm

echo "Web app built in $out"
