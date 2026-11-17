# Plantillas personalizadas

Una plantilla es un archivo [Typst](https://typst.app/docs) que lee `data.json` y decide el diseño. Todos los valores llegan ya formateados para imprimir (fechas `dd/mm/aaaa`, importes con símbolo y separador de miles), así que la plantilla solo los coloca.

## Empezar

1. Copia una plantilla integrada: `xml2pdf template --format a4 > mi-plantilla.typ` (o `ticket80`).
2. Mira los datos de un comprobante real: `xml2pdf data F001-123.xml`.
3. Prueba el resultado: `xml2pdf convert F001-123.xml --template mi-plantilla.typ`.

Si la plantilla tiene un error, el mensaje indica la línea.

## Archivos disponibles

| Archivo | Contenido |
|---------|-----------|
| `data.json` | Datos del comprobante (ver abajo) |
| `qr.svg` | Código QR; existe solo cuando `qr` no es nulo |
| `logo.*` | Logo configurado; su nombre llega en `logo` |

La fuente incluida es **Noto Sans** (normal y negrita).

## Campos de `data.json`

| Campo | Contenido |
|-------|-----------|
| `format` | `a4`, `a5`, `ticket80` o `ticket58` |
| `page.width`, `page.ticket` | Ancho del papel en mm y si es papel continuo |
| `logo`, `qr` | Nombre del archivo de imagen, o nulo |
| `doc.title`, `doc.id`, `doc.type_code` | Título (`FACTURA ELECTRÓNICA`), serie y número, código de tipo |
| `doc.issue_date`, `doc.issue_time`, `doc.due_date` | Fechas y hora |
| `doc.currency`, `doc.currency_name`, `doc.currency_symbol` | `PEN`, `SOLES`, `S/` |
| `doc.fields` | Lista `{label, value}` con los datos del comprobante listos para mostrar |
| `issuer.ruc`, `issuer.name`, `issuer.trade_name`, `issuer.address`, `issuer.extra` | Emisor; `extra` son las líneas adicionales configuradas |
| `customer.doc_label`, `customer.doc_number`, `customer.name`, `customer.address` | Cliente |
| `customer.fields` | Lista `{label, value}` con los datos del cliente |
| `lines[]` | Ítems: `number`, `code`, `description`, `unit` (abreviatura), `unit_code` (código SUNAT), `quantity`, `unit_value` (sin impuestos), `unit_price` (con impuestos), `discount`, `value` (sin impuestos), `tax`, `total` (con impuestos), `free` |
| `totals[]` | Filas `{label, value, amount}` en orden de impresión; solo las que tienen importe. `value` incluye el símbolo de moneda y `amount` no |
| `total` | `{label, value, amount}` del importe total |
| `amount_in_words` | `… CON 00/100 SOLES`, sin el prefijo `SON:` |
| `legends[]`, `notes[]` | Leyendas y observaciones |
| `payment` | Nulo o `{form, pending, installments[]}`; cada cuota tiene `number`, `due_date`, `amount` |
| `detraction` | Nulo o `{code, percent, amount, account}` |
| `affected` | Nulo o `{document, reason, description}` (notas de crédito y débito) |
| `related[]` | Documentos relacionados `{label, id}` |
| `hash` | Resumen de la firma del XML |
| `legal`, `footer` | Texto legal y texto al pie configurado |
| `credit.text`, `credit.links[]` | Crédito de la herramienta y sus direcciones, para mostrarlas como enlaces |

Los campos opcionales llegan como nulo (`none` en Typst) cuando el XML no los trae.
