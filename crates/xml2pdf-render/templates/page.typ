// xml2pdf built-in template for A4 and A5 pages.
//
// Black and gray only, laid out like the usual printed invoice in Peru:
// issuer and document box, customer and dates, items, totals, amount in
// words, legal text and QR code.
//
// Copy this file to start a custom template. Everything printed comes from
// `data.json`; its fields are described in docs/templates.md.

#let d = json("data.json")
#let small = d.format == "a5"
#let base = if small { 6.5pt } else { 8pt }
#let border = 0.6pt + luma(110)
#let shade = luma(232)
#let stripe = luma(240)
#let inset = if small { 2mm } else { 3mm }
#let gap = 1.5mm
// Width of the right-hand boxes (document number and dates).
#let side = if small { 4.8cm } else { 7cm }
// The items table is padded with empty rows up to this many.
#let min-rows = if small { 6 } else { 8 }

#set document(title: d.doc.title + " " + d.doc.id, author: d.issuer.name)
#set page(paper: d.format, margin: if small { 8mm } else { 12mm })
#set text(font: "Noto Sans", size: base, lang: "es")
#set par(leading: 0.45em, spacing: 0.6em)
#set block(spacing: gap)

#let strong(body) = text(weight: "bold", body)
#let present(value) = value != none and value != ""
#let panel(body, fill: none, height: auto) = block(
  width: 100%,
  height: height,
  stroke: border,
  radius: 4pt,
  inset: inset,
  fill: fill,
  body,
)
// Rows of `LABEL : value`, given as `(label, value)` pairs. Empty values are skipped.
#let rows(items, label-width: auto, label-align: left) = grid(
  columns: (label-width, auto, 1fr),
  align: (label-align, center, left),
  column-gutter: 1.5mm,
  row-gutter: 1.3mm,
  ..items.filter(item => present(item.at(1))).map(item => (strong(item.at(0)), [:], item.at(1))).flatten(),
)
// Two boxes side by side with the same height; the right one is `width` wide.
#let pair(left, right, width: side) = layout(size => {
  let left-width = size.width - width - gap
  let height = calc.max(
    measure(width: left-width - 2 * inset, left).height,
    measure(width: width - 2 * inset, right).height,
  ) + 2 * inset
  grid(
    columns: (left-width, width),
    column-gutter: gap,
    panel(height: height, left), panel(height: height, right),
  )
})

// Issuer on the left, document identification on the right.
#grid(
  columns: (1fr, side),
  column-gutter: if small { 4mm } else { 8mm },
  align: (left + top, center + horizon),
  {
    if present(d.logo) {
      image(d.logo, height: if small { 1.2cm } else { 1.8cm })
      v(1mm)
    }
    strong(d.issuer.name)
    let details = (d.issuer.trade_name, d.issuer.address, ..d.issuer.extra).filter(present)
    if details.len() > 0 {
      linebreak()
      text(size: base - 0.5pt, details.join(linebreak()))
    }
  },
  panel(fill: shade, {
    set par(leading: 0.5em)
    text(size: if small { base + 4pt } else { base + 8pt }, weight: "bold", {
      [RUC #d.issuer.ruc]
      linebreak()
      // The title goes on two lines: `FACTURA` / `ELECTRÓNICA`.
      let title = d.doc.title.split(" ELECTRÓNICA")
      title.first()
      if title.len() > 1 {
        linebreak()
        [ELECTRÓNICA]
      }
      linebreak()
      d.doc.id
    })
  }),
)

#v(1mm)

// Customer and dates.
#pair(
  {
    strong[DATOS DEL CLIENTE]
    v(0.3mm)
    rows((
      (d.customer.doc_label, d.customer.doc_number),
      ("DENOMINACIÓN", d.customer.name),
      ("DIRECCIÓN", d.customer.address),
    ))
  },
  rows((
    ("FECHA EMISIÓN", d.doc.issue_date),
    ("FECHA DE VENC.", d.doc.due_date),
    ("MONEDA", d.doc.currency_name),
  )),
)

// Document modified by a credit or debit note.
#if d.affected != none {
  panel(rows((
    ("DOCUMENTO QUE MODIFICA", d.affected.document),
    ("MOTIVO", (d.affected.reason, d.affected.description).filter(present).join(" - ")),
  )))
}

// Items. The heading is a separate box so it can have rounded corners; both
// use the same column widths.
#let widths = (9%, 8%, 10%, 1fr, 12%, 12%, 13%)
#let aligns = (center, center, center, left, right, right, right)

#block(
  width: 100%,
  stroke: border,
  radius: 4pt,
  fill: shade,
  grid(
    columns: widths,
    align: aligns,
    inset: (x: 1.5mm, y: 1.7mm),
    ..("CANT.", "UM", "CÓD.", "DESCRIPCIÓN", "V/U", "P/U", "IMPORTE").map(strong),
  ),
)
#table(
  columns: widths,
  align: aligns,
  stroke: (x: none, y: 0.4pt + luma(150)),
  inset: (x: 1.5mm, y: 1.2mm),
  fill: (_, row) => if calc.odd(row) { stripe },
  ..d.lines.map(line => (
    line.quantity,
    line.unit_code,
    line.code,
    line.description,
    line.unit_value,
    line.unit_price,
    line.total,
  )).flatten(),
  ..range(calc.max(0, min-rows - d.lines.len()) * widths.len()).map(_ => [~]),
)

// Totals, with the currency symbol in its own column.
#align(right, block(width: side, inset: (right: 1.5mm), grid(
  columns: (1fr, auto, if small { 1.8cm } else { 2.4cm }),
  align: (right, center, right),
  column-gutter: if small { 3mm } else { 5mm },
  row-gutter: 1.2mm,
  ..(..d.totals, d.total).map(row => (
    strong(row.label),
    strong(d.doc.currency_symbol),
    strong(row.amount),
  )).flatten(),
)))

#panel(pad(left: 8%, [#strong[IMPORTE EN LETRAS] : #d.amount_in_words]))

// Related documents, payment and remarks.
#let payment = if d.payment != none { d.payment } else { (form: none, pending: none, installments: ()) }
#let detraction = if d.detraction != none {
  (
    d.detraction.percent,
    d.detraction.amount,
    if present(d.detraction.account) { "Cuenta Banco de la Nación " + d.detraction.account },
  ).filter(present).join(" · ")
}
#let details = (
  ..d.related.map(item => (item.label, item.id)),
  ("ORDEN DE COMPRA", d.doc.order_reference),
  ("FORMA DE PAGO", payment.form),
  ("PENDIENTE DE PAGO", payment.pending),
  ..payment.installments.map(item => (
    "CUOTA " + item.number,
    (item.due_date, item.amount).filter(present).join(" · "),
  )),
  ("DETRACCIÓN", detraction),
  ("OBSERVACIONES", d.notes.join(linebreak())),
).filter(item => present(item.at(1)))
#if details.len() > 0 {
  block(inset: (y: 1mm), rows(details, label-width: 36%, label-align: right))
}

// Legal text and QR code.
#let legal = {
  d.legal
  for legend in d.legends {
    linebreak()
    legend
  }
  if present(d.footer) {
    linebreak()
    d.footer
  }
  if present(d.hash) {
    linebreak()
    [#strong[Resumen:] #d.hash]
  }
}
#if present(d.qr) {
  pair(legal, image(d.qr, width: 100%), width: if small { 2.6cm } else { 3.4cm })
} else {
  panel(legal)
}

// Credit of the tool, with its addresses as links.
#align(center, text(size: base - 1.5pt, fill: luma(90), {
  d.credit.text
  [ ]
  d.credit.links.map(address => link(address)).join(" o ")
}))
