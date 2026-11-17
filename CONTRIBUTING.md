# Contribuir a xml2pdf

Gracias por tu interés. Esta guía te lleva de clonar el repositorio a enviar un cambio.

## Camino rápido

1. Haz un fork y crea una rama desde `main`: `git switch -c fix/descripcion-corta`.
2. Haz el cambio con su prueba.
3. Ejecuta las comprobaciones de la tabla de abajo.
4. Abre un pull request describiendo qué cambia y cómo lo probaste.

## Preparar el entorno

| Para trabajar en | Necesitas |
|------------------|-----------|
| Núcleo y CLI | [Rust](https://rustup.rs) 1.92 o superior |
| Web | Lo anterior, el target `wasm32-unknown-unknown` y `wasm-bindgen-cli` de la misma versión que `wasm-bindgen` en `Cargo.lock` |
| Prueba de la interfaz | Lo anterior, Node.js, Python 3 y Firefox |
| Escritorio | Lo anterior y las [dependencias de Tauri 2](https://tauri.app/start/prerequisites/) |

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --locked --version <versión de Cargo.lock>
```

## Comprobaciones

Son las mismas que ejecuta el CI. Un pull request necesita todas en verde.

| Comprobación | Comando |
|--------------|---------|
| Formato | `cargo fmt --all --check` |
| Lints | `cargo clippy --locked --all-targets -- -D warnings` |
| Pruebas | `cargo test --locked` |
| Compilar la web | `./scripts/build-web.sh` |
| Compilar la interfaz del escritorio | `./scripts/build-desktop-ui.sh` |
| Motor en WebAssembly | `node scripts/smoke-web.mjs dist/web fixtures` |
| Interfaz en navegador | `python3 scripts/ui-check.py` |
| Escritorio | `cargo clippy --locked -p xml2pdf-desktop --all-targets -- -D warnings` y `cargo test --locked -p xml2pdf-desktop` |

Si tu cambio solo toca el núcleo o el CLI, bastan las tres primeras.

## Reglas del proyecto

| Regla | Detalle |
|-------|---------|
| La lógica vive en el núcleo | Las aplicaciones (`cli`, `wasm`, `apps`) solo conectan. Si una regla del comprobante aparece en JavaScript o en el CLI, va en el lugar equivocado. |
| El núcleo no hace entrada ni salida | `xml2pdf-core` y `xml2pdf-render` deben seguir compilando para WebAssembly. |
| Cada cambio de comportamiento trae su prueba | La prueba entra por la interfaz pública y usa los comprobantes de `fixtures`. |
| Sin `unsafe` | El espacio de trabajo lo prohíbe. |
| Código, comentarios y commits en inglés | La documentación y los textos de la interfaz, en español. |
| Si cambias un comando u opción | Actualiza su ayuda y el README. |

Antes de un cambio grande, lee [docs/architecture.md](docs/architecture.md): explica dónde va cada cosa y cómo agregar un comprobante, un bloque o un formato.

## Commits y pull requests

Los commits siguen [Conventional Commits](https://www.conventionalcommits.org/es/): `feat(render): add a watermark block`, `fix(core): read amounts with a thousands separator`.

| Tipo | Úsalo para |
|------|------------|
| `feat` | Una función nueva |
| `fix` | La corrección de un error |
| `docs` | Solo documentación |
| `test` | Solo pruebas |
| `refactor` | Reorganizar sin cambiar el comportamiento |
| `build`, `ci`, `chore` | Compilación, automatización y mantenimiento |

Prefiere commits pequeños, cada uno con una sola idea, y pull requests que se puedan revisar de una sentada.

## Reportar un XML que falla

Es la contribución más útil. Para que se pueda reproducir:

1. Reemplaza los datos reales del XML: RUC, nombres, direcciones y montos. Los comprobantes contienen datos de tu empresa y de tus clientes.
2. Comprueba que el XML modificado sigue fallando.
3. Abre un reporte con el XML, el formato elegido y lo que esperabas ver.

## Seguridad

No abras un reporte público para una vulnerabilidad. Sigue [SECURITY.md](SECURITY.md).

## Licencia

Al contribuir aceptas que tu aporte se publique bajo la [licencia MIT](LICENSE) del proyecto.
