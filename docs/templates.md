# Diseños y plantillas

Hay dos formas de cambiar cómo se ve el PDF:

| Forma | Para qué | Dificultad |
|-------|----------|------------|
| **Diseño por bloques** | Reordenar, ocultar y ajustar las partes del comprobante | Se hace en el diseñador, sin escribir código |
| **Plantilla personalizada** | Control total del dibujo | Requiere escribir Typst |

## Diseños por bloques

Un diseño es una lista de filas; cada fila tiene de uno a tres bloques lado a lado. El diseñador lo exporta así, listo para usar con `--config`:

```json
{
  "page_design": {
    "style": { "text": "normal", "boxes": "rounded", "heading": "shaded", "stripes": true, "filler_rows": true },
    "rows": [
      { "blocks": [{ "type": "issuer", "width": 3 }, { "type": "document", "width": 2 }] },
      { "blocks": [{ "type": "items", "columns": ["quantity", "description", "total"] }] },
      { "blocks": [{ "type": "text", "text": "Gracias por su compra", "align": "center" }] }
    ]
  },
  "ticket_design": null
}
```

`page_design` vale para A4, A5 y Carta; `ticket_design`, para los tickets. Con `null` se usa el diseño Clásico.

| Estilo | Valores |
|--------|---------|
| `text` | `small`, `normal`, `large` |
| `boxes` | `rounded`, `square`, `lines`, `none` |
| `heading` | `shaded`, `inverted`, `plain` |
| `stripes`, `filler_rows` | `true` o `false` |

| Bloque (`type`) | Contenido |
|-----------------|-----------|
| `issuer` | Logo, emisor, dirección y datos adicionales |
| `document` | RUC, tipo de comprobante y número |
| `customer`, `dates` | Datos del cliente; fechas y moneda |
| `affected` | Documento que modifica (solo en notas) |
| `items` | Tabla de ítems |
| `totals`, `words` | Totales; importe en letras |
| `details` | Guías, forma de pago, cuotas y observaciones |
| `legal`, `qr`, `credit` | Texto legal y resumen; código QR; crédito de la herramienta |
| `text`, `rule`, `spacer` | Texto libre; línea; espacio |

| Opción de un bloque | Valores |
|---------------------|---------|
| `width` | 1 a 6: ancho relativo dentro de su fila |
| `boxed` | `true` o `false`; si falta, lo decide el estilo |
| `align` | `left`, `center`, `right` |
| `text` | Texto de un bloque `text` (hasta 600 caracteres) |
| `columns` | Columnas de `items`: `quantity`, `unit_code`, `unit`, `code`, `description`, `unit_value`, `unit_price`, `discount`, `value`, `tax`, `total` |

Un bloque sin nada que mostrar (por ejemplo `affected` en una factura) se omite solo.

# Plantillas personalizadas

Una plantilla es un archivo [Typst](https://typst.app/docs) que lee `data.json` y decide el diseño. Todos los valores llegan ya formateados para imprimir (fechas `dd/mm/aaaa`, importes con símbolo y separador de miles), así que la plantilla solo los coloca.

## Empezar

1. Copia la plantilla integrada: `xml2pdf template > mi-plantilla.typ`. Una sola plantilla dibuja todos los formatos y diseños.
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
| `format` | `a4`, `a5`, `letter`, `ticket80` o `ticket58` |
| `page.width`, `page.ticket`, `page.paper` | Ancho del papel en mm, si es papel continuo y nombre del papel en Typst |
| `design` | Diseño por bloques ya resuelto: `style` y `rows[].blocks[]` con `id`, `type`, `width`, `boxed`, `align`, `text`, `columns` |
| `editor` | `true` solo cuando el diseñador pide la posición de los bloques |
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
