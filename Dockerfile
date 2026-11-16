# Web app image: builds the WebAssembly converter and serves it as static files.
# Used by Dokploy (build type: Dockerfile, container port: 80).

FROM rust:1-bookworm AS build
RUN rustup target add wasm32-unknown-unknown
WORKDIR /src

# The wasm-bindgen CLI must match the wasm-bindgen crate, so its version is
# read from Cargo.lock. This layer is rebuilt only when the lock file changes.
COPY Cargo.lock ./
RUN cargo install wasm-bindgen-cli --locked \
    --version "$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/[^0-9.]/, ""); print; exit }' Cargo.lock)"

COPY . .
RUN ./scripts/build-web.sh /out \
    && find /out -type f \( -name '*.wasm' -o -name '*.js' -o -name '*.css' -o -name '*.html' -o -name '*.svg' -o -name '*.ttf' \) \
        -exec gzip -9 --keep {} +

FROM caddy:2-alpine
COPY deploy/Caddyfile /etc/caddy/Caddyfile
COPY --from=build /out /srv
EXPOSE 80
HEALTHCHECK --interval=30s --timeout=3s --retries=3 \
    CMD wget --quiet --spider http://127.0.0.1/healthz || exit 1
