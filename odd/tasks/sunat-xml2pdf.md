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

- [ ] **T1** (S1, S2) Workspace scaffold, domain model and UBL parser in `crates/xml2pdf-core`. Route: inline. Commit: pending.
- [ ] **T2** (S1, S5) PDF rendering in `crates/xml2pdf-render`: Typst world, built-in templates for the four formats, QR, render config. Route: inline. Commit: pending.
- [ ] **T3** (S1, S5) CLI `xml2pdf` in `crates/xml2pdf-cli`: single file, batch, ZIP input, config and custom template. Route: inline. Commit: pending.
- [ ] **T4** (S3, S5) Web: `crates/xml2pdf-wasm` bindings and static UI in `apps/web`. Route: inline. Commit: pending.
- [ ] **T5** (S4) Desktop: Tauri 2 app in `apps/desktop` with folder watcher and auto-print. Route: inline. Commit: pending.
- [ ] **T6** (S6) Deploy: `deploy/Dockerfile`, Caddy config, compose file, GitHub Actions CI and release workflows. Route: inline. Commit: pending.
- [ ] **T7** (S7) Tests for parser, renderer, CLI and desktop logic; README and docs. Route: inline. Commit: pending.

Route evidence: no writer is delegated. The units share `Cargo.toml`, `Cargo.lock` and one `target` directory, T4 and T5 depend on the T1/T2 API, and the WASM toolchain needs a parent-owned system install.

## Log

- **L1** (2026-11-16, user, verbatim) "Quiero hacer una herramienta, opensource para poder convertir xml de sunat a pdf para que sea imprimible  eso con rust talvez, par aque sea multiplataforma, una version web par que se pueda generar de la misma manera pero otra verison tambien de escritorio para autoimprimir todo, esto con el fin de para quienen facturan desde plataforma sunat tengan opcion de tener pdf imprimibles y configurables editables en diferentes formatos, dame la reocmendacion, esto debe poder depslegarse con gi gd a un dokploy"
- **L2** (2026-11-16, user, verbatim) "No hazlo todo completo primero planifica las pruebas para el final por que haremos todo de una pasada"
- **L3** (2026-11-16, decision) Stack: Rust core, Typst as the PDF engine, `roxmltree` for XML (tree navigation with namespaces; replaces the earlier `quick-xml` suggestion), wasm-bindgen for web, Tauri 2 for desktop, Caddy static container for Dokploy. License: MIT (default, open to change).
- **L4** (2026-11-16, environment) Local machine: Rust 1.99.1 without the wasm32 target (`rust-wasm` package needs sudo; three sudo attempts failed, the user will install it), no Docker/Podman, no CUPS (`lp`). Consequence: the WASM build, the Docker image and real printing cannot be verified locally until those exist. RDD is off (global).
