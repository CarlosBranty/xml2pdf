# xml2pdf

Convierte comprobantes electrónicos de SUNAT (XML UBL 2.1) en PDF listos para imprimir: desde la terminal, desde el navegador o de forma automática con la aplicación de escritorio. Es software libre (MIT).

> El PDF es una representación impresa. El comprobante con validez tributaria sigue siendo el XML.

## Uso rápido

```bash
cargo install --path crates/xml2pdf-cli      # instala el comando `xml2pdf`
xml2pdf convert F001-123.xml                 # crea F001-123.pdf junto al XML
xml2pdf convert descargas/ -o pdf/ -f ticket80
```

## Qué incluye

| Pieza | Para qué sirve | Dónde está |
|-------|----------------|------------|
| CLI `xml2pdf` | Convertir archivos o carpetas, vigilar una carpeta e imprimir | `crates/xml2pdf-cli` |
| Web | Convertir en el navegador; los archivos no salen de tu equipo | `apps/web` |
| Escritorio | Vigilar la carpeta de descargas e imprimir cada comprobante nuevo | `apps/desktop` |

En la web: pega el contenido del XML, arrástralo o elige el archivo; presiona **Convertir** y verás la vista previa en los cuatro formatos, cada una con **Guardar** e **Imprimir**. En Chrome y Edge, Guardar deja elegir la carpeta; en otros navegadores descarga el archivo.

También puedes convertir por lotes: elige o arrastra varios XML (o un ZIP) y usa **Guardar todos** para descargar un ZIP con todos los PDF en el formato que elijas. Cada tanda admite hasta 100 comprobantes y 50 MB; lo que exceda se omite con un aviso.

Comprobantes admitidos: factura (01), boleta de venta (03), nota de crédito (07) y nota de débito (08). La entrada puede ser un XML (UTF-8 o ISO-8859-1) o un ZIP con varios; los archivos de respuesta de SUNAT (`R-…xml`) se omiten.

## Formatos y personalización

| Opción | Valores | CLI |
|--------|---------|-----|
| Tamaño de papel | `a4`, `a5`, `ticket80`, `ticket58` | `--format` |
| Logo | PNG, JPEG, GIF, WebP o SVG | `--logo` |
| Texto al pie | texto libre | `--footer` |
| Código QR | visible u oculto | `--no-qr` |
| Plantilla propia | archivo Typst | `--template` |

Las mismas opciones se pueden guardar en un archivo JSON y usar con `--config`; las opciones de la línea de comandos tienen prioridad.

```json
{
  "format": "a4",
  "issuer_lines": ["Tel. 01 555 0100", "ventas@ejemplo.pe"],
  "footer": "Gracias por su preferencia",
  "show_qr": true,
  "logo": "logo.png",
  "template": "mi-plantilla.typ"
}
```

Todos los PDF terminan con una línea de crédito de la herramienta, con enlaces a qlab.pe y qodi.pe.

Para cambiar el diseño, parte de una plantilla integrada: `xml2pdf template --format a4 > mi-plantilla.typ`. Los datos disponibles están en [docs/templates.md](docs/templates.md).

## Impresión automática

```bash
xml2pdf printers                                   # impresoras disponibles
xml2pdf watch ~/Descargas --print --printer Caja   # convierte e imprime cada XML nuevo
```

La aplicación de escritorio muestra la misma interfaz que la web y añade la sección **Impresión automática**: allí eliges la carpeta a vigilar, la impresora y las copias. En el escritorio, Guardar abre el diálogo del sistema e Imprimir envía el PDF directo a esa impresora. Los PDF se guardan en la subcarpeta `pdf` de la carpeta vigilada, salvo que elijas otra.

| Sistema | Cómo imprime | Requisito |
|---------|--------------|-----------|
| Linux y macOS | `lp` (CUPS) | CUPS instalado y una impresora configurada |
| Windows | SumatraPDF en modo silencioso | [SumatraPDF](https://www.sumatrapdfreader.org) instalado; sin él se usa la aplicación PDF predeterminada, que puede mostrar su ventana |

## Compilar

Requiere Rust estable.

| Objetivo | Comando | Requisitos adicionales |
|----------|---------|------------------------|
| Pruebas | `cargo test` | — |
| Web | `./scripts/build-web.sh` (genera `dist/web`) | target `wasm32-unknown-unknown` y `wasm-bindgen-cli` de la misma versión que en `Cargo.lock` |
| Prueba de la web en navegador | `python3 scripts/ui-check.py` (tras compilar la web) | Firefox |
| Escritorio | `./scripts/build-desktop-ui.sh` y luego `cargo run -p xml2pdf-desktop` (o `cd apps/desktop && npm install && npm run build` para el instalador) | [Dependencias de Tauri 2](https://tauri.app/start/prerequisites/) |

## Desplegar la web en Dokploy

1. Crea una aplicación en Dokploy con este repositorio, tipo de build **Dockerfile** (ruta `./Dockerfile`).
2. Añade un dominio apuntando al puerto **80** del contenedor.
3. Define el argumento de build `SITE_URL` con la dirección pública (sin barra final). Con él se publican el enlace canónico, la imagen para redes y el `sitemap.xml`; sin él, esas partes se omiten.
4. Para desplegar desde CI, define en GitHub los secretos `DOKPLOY_URL`, `DOKPLOY_API_KEY` y `DOKPLOY_APPLICATION_ID`. El workflow `CI` despliega cada push a `main` después de pasar las pruebas; sin esos secretos, el paso se omite. En ese caso desactiva **Auto Deploy** en Dokploy, o cada push se desplegará dos veces (una antes de que termine el CI).

La imagen compila el motor WebAssembly en el servidor: el primer build tarda varios minutos y consume bastante memoria.

La política del sitio solo permite incrustarlo en un marco desde `qlab.pe`; para otro dominio, cambia `frame-ancestors` en `deploy/Caddyfile`.

Para probar la imagen en local: `docker compose up --build` y abre <http://localhost:8080>.

Los instaladores de escritorio y los binarios del CLI se publican al subir una etiqueta `vX.Y.Z` (workflow `Release`).

## Estructura

| Carpeta | Contenido |
|---------|-----------|
| `crates/xml2pdf-core` | Modelo del comprobante y lector UBL 2.1 |
| `crates/xml2pdf-render` | Plantillas Typst y generación del PDF |
| `crates/xml2pdf-auto` | Vigilancia de carpetas e impresión |
| `crates/xml2pdf-wasm` | Enlaces WebAssembly para la web |
| `fixtures` | Comprobantes de ejemplo usados en las pruebas |
