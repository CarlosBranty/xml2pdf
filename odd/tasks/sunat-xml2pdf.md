# sunat-xml2pdf

Objective: open-source tool that converts SUNAT electronic documents (UBL 2.1 XML) into printable, configurable PDFs, shipped as CLI, web (WASM) and desktop (auto-print).
Branch: `feat/sunat-xml2pdf`. Delivery strategy: `single-pr` (user: "haremos todo de una pasada"). Forecast: well above 400 authored lines; accepted by the user's single-pass instruction.
Test runner: `cargo test --workspace` (tests are written last by explicit user instruction, see S7).

## Specs

- **S1** "herramienta, opensource para poder convertir xml de sunat a pdf para que sea imprimible". Parse SUNAT UBL 2.1 XML and produce a PDF "representación impresa" including QR code and signature hash. Assumption (from the accepted recommendation): document types are factura (01), boleta (03), nota de crédito (07) and nota de débito (08). ZIP files containing the XML are accepted as input (assumption: SUNAT downloads are often zipped).
- **S2** "con rust talvez, par aque sea multiplataforma". Rust workspace; the same core runs on Linux, macOS, Windows and WASM.
- **S3** "una version web par que se pueda generar de la misma manera". Static web app; the PDF is generated in the browser by the same core compiled to WASM. No backend, the XML never leaves the browser.
- **S4** "otra verison tambien de escritorio para autoimprimir todo". Desktop app (Tauri 2) that watches a folder and automatically converts and prints every new XML.
- **S5** "tengan opcion de tener pdf imprimibles y configurables editables en diferentes formatos". Formats: A4, A5, ticket 80 mm, ticket 58 mm. Configurable: logo, accent color, extra issuer texts, footer. Editable: templates are plain Typst files and a custom template can replace the built-in one.
- **S6** "esto debe poder depslegarse con gi gd a un dokploy". Assumption: "gi gd" means CI/CD. Dockerfile serving the static web build, plus a CI workflow and a Dokploy deploy trigger.
- **S7** "primero planifica las pruebas para el final por que haremos todo de una pasada". Plan first; implement everything in one pass; tests are written at the end (overrides the default test-first policy).

## Tasks

- [x] **T1** (S1, S2) Workspace scaffold, domain model and UBL parser in `crates/xml2pdf-core`. Route: inline. Commit: 51b2e95.
- [x] **T2** (S1, S5) PDF rendering in `crates/xml2pdf-render`: Typst world, built-in templates for the four formats, QR, render config. Route: inline. Commit: c32b67f.
- [x] **T3** (S1, S5) CLI `xml2pdf` in `crates/xml2pdf-cli`: single file, batch, ZIP input, config and custom template. Route: inline. Commit: d947fd5.
- [x] **T4** (S3, S5) Web: `crates/xml2pdf-wasm` bindings and static UI in `apps/web`. Route: inline. Commit: 6b9f95a.
- [x] **T5** (S4) Desktop: Tauri 2 app in `apps/desktop` with folder watcher and auto-print; shared logic in `crates/xml2pdf-auto`, also exposed as `xml2pdf watch` and `xml2pdf printers`. Route: inline. Commit: fea32cf.
- [x] **T6** (S6) Deploy: `deploy/Dockerfile`, Caddy config, compose file, GitHub Actions CI and release workflows. Route: inline. Commit: 7db6ffb.
- [x] **T7** (S7) Tests for parser, renderer, CLI and desktop logic; README and docs. Route: inline. Commit: 5067815.

Route evidence: no writer is delegated. The units share `Cargo.toml`, `Cargo.lock` and one `target` directory, T4 and T5 depend on the T1/T2 API, and the WASM toolchain needs a parent-owned system install.

## Log

- **L1** (2026-11-16, user, verbatim) "Quiero hacer una herramienta, opensource para poder convertir xml de sunat a pdf para que sea imprimible  eso con rust talvez, par aque sea multiplataforma, una version web par que se pueda generar de la misma manera pero otra verison tambien de escritorio para autoimprimir todo, esto con el fin de para quienen facturan desde plataforma sunat tengan opcion de tener pdf imprimibles y configurables editables en diferentes formatos, dame la reocmendacion, esto debe poder depslegarse con gi gd a un dokploy"
- **L2** (2026-11-16, user, verbatim) "No hazlo todo completo primero planifica las pruebas para el final por que haremos todo de una pasada"
- **L3** (2026-11-16, decision) Stack: Rust core, Typst as the PDF engine, `roxmltree` for XML (tree navigation with namespaces; replaces the earlier `quick-xml` suggestion), wasm-bindgen for web, Tauri 2 for desktop, Caddy static container for Dokploy. License: MIT (default, open to change).
- **L4** (2026-11-16, environment) Local machine: Rust 1.99.1 without the wasm32 target (`rust-wasm` package needs sudo; three sudo attempts failed, the user will install it), no Docker/Podman, no CUPS (`lp`). Consequence: the WASM build, the Docker image and real printing cannot be verified locally until those exist. RDD is off (global).
- **L5** (2026-11-16, evidence) T1-T3: the CLI converted every fixture (factura, ISO-8859-1 boleta, credit note, USD debit note, ZIP with a CDR entry skipped) in A4, A5, ticket80 and ticket58; A4, A5 and both ticket layouts were inspected as rendered images. `cargo clippy` clean.
- **L6** (2026-11-16, environment) The user installed `rust-wasm`; the wasm32 target is available. `wasm-bindgen-cli` 0.2.130 is installed project-locally in `target/tools`.
- **L7** (2026-11-16, evidence) T4: `scripts/build-web.sh` builds `dist/web` (WASM 24.3 MB, 8.6 MB gzipped); `node scripts/smoke-web.mjs dist/web fixtures` converts all fixtures inside the WebAssembly module and passes. The browser UI itself (`apps/web/app.js`) was only syntax-checked, not exercised in a browser.
- **L8** (2026-11-16, evidence) T5: `xml2pdf watch` converted new files end to end in a temp folder; the desktop binary builds and stays up for a 7 s smoke launch. Not verified: window interaction (IPC from the UI), real printing (no CUPS here), anything on Windows or macOS.
- **L9** (2026-11-16, evidence) T6: Dockerfile, Caddyfile and workflows are written but not executed (no Docker, Caddy or runner locally). The Dokploy deploy call follows docs.dokploy.com (`POST /api/application.deploy`, `x-api-key`).
- **L10** (2026-11-16, evidence) T7: `cargo test --workspace --locked`: 55 passed, 0 failed (core 12, render 16, auto 13 + 4 unit, cli 10). `cargo clippy --workspace --all-targets --locked -- -D warnings` clean. Tests were written after the code (S7), so no RED run was observed.
- **L11** (2026-11-16, decision) UI copy and README are in Spanish because the audience is SUNAT taxpayers; code, comments and commits stay in English.
- **L12** (2026-11-16, risk) Risk: item 5 (CI, Docker image and deploy trigger are new and could not be run locally). Independent verification requested for T6.
