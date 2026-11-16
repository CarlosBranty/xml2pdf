// xml2pdf built-in template for A4 and A5 pages.
//
// Copy this file to start a custom template. Everything printed comes from
// `data.json`; its fields are described in docs/templates.md.

#let d = json("data.json")
#let accent = rgb(d.accent)
#let small = d.format == "a5"
#let base = if small { 7pt } else { 9pt }
#let muted = luma(90)

#set document(title: d.doc.title + " " + d.doc.id, author: d.issuer.name)
#set page(paper: d.format, margin: if small { 9mm } else { 14mm })
#set text(font: "Noto Sans", size: base, lang: "es")
#set par(leading: 0.5em, spacing: 0.7em)

#let strong(body) = text(weight: "bold", body)
#let present(value) = value != none and value != ""
#let fields(items, value-width: 1fr) = grid(
  columns: (auto, value-width),
  column-gutter: 2mm,
  row-gutter: 1.5mm,
  ..items.map(item => (strong(item.label + ":"), item.value)).flatten(),
)
#let panel(body) = block(
  width: 100%,
  stroke: 0.5pt + luma(190),
  radius: 3pt,
  inset: if small { 2.5mm } else { 3.5mm },
  body,
)

// Issuer and document identification.
#grid(
  columns: (1fr, if small { 4.8cm } else { 6.6cm }),
  column-gutter: if small { 4mm } else { 8mm },
  align: (left + horizon, center + horizon),
  {
    if present(d.logo) {
      image(d.logo, height: if small { 1.3cm } else { 1.9cm })
      v(1.5mm)
    }
    text(size: base + 3pt, weight: "bold", d.issuer.name)
    let details = (d.issuer.trade_name, d.issuer.address, ..d.issuer.extra).filter(present)
    if details.len() > 0 {
      linebreak()
      text(fill: muted, details.join(linebreak()))
    }
  },
  block(width: 100%, stroke: 1.2pt + accent, radius: 4pt, inset: if small { 3mm } else { 4.5mm }, {
    text(size: base + 2pt, weight: "bold")[RUC #d.issuer.ruc]
    v(1mm)
    block(width: 100%, fill: accent, inset: 2mm, radius: 2pt, text(fill: white, weight: "bold", size: base + 1pt, d.doc.title))
    v(1mm)
    text(size: base + 3pt, weight: "bold", d.doc.id)
  }),
)

#v(2mm)

// Customer and document data.
#panel(grid(
  columns: (1fr, auto),
  column-gutter: 6mm,
  fields(d.customer.fields), fields(d.doc.fields, value-width: auto),
))

// Document modified by a credit or debit note.
#if d.affected != none {
  panel(fields((
    (label: "DOCUMENTO QUE MODIFICA", value: d.affected.document),
    (label: "MOTIVO", value: (d.affected.reason, d.affected.description).filter(present).join(" - ")),
  )))
}

// Items.
#table(
  columns: (auto, auto, auto, 1fr, auto, auto, auto),
  align: (right, center, left, left, right, right, right),
  stroke: none,
  inset: (x: 2mm, y: 1.7mm),
  fill: (_, row) => if row == 0 { accent } else if calc.even(row) { luma(244) },
  table.header(
    ..("CANT.", "UND", "CÓDIGO", "DESCRIPCIÓN", "V. UNIT.", "P. UNIT.", "VALOR VENTA").map(title => text(
      fill: white,
      weight: "bold",
      size: base - 1pt,
      title,
    )),
  ),
  ..d.lines.map(line => (
    line.quantity,
    line.unit,
    line.code,
    line.description,
    line.unit_value,
    line.unit_price,
    line.value,
  )).flatten(),
)

#line(length: 100%, stroke: 0.6pt + accent)

// Amount in words, notes and totals.
#grid(
  columns: (1fr, auto),
  column-gutter: 6mm,
  {
    strong(d.amount_in_words)
    for legend in d.legends {
      parbreak()
      legend
    }
    if d.notes.len() > 0 {
      parbreak()
      strong[OBSERVACIONES: ]
      d.notes.join(linebreak())
    }
    if d.payment != none and d.payment.installments.len() > 0 {
      v(1mm)
      if present(d.payment.pending) [#strong[MONTO PENDIENTE DE PAGO:] #d.payment.pending]
      table(
        columns: 3,
        align: (center, center, right),
        stroke: 0.4pt + luma(190),
        inset: 1.4mm,
        strong[CUOTA], strong[VENCIMIENTO], strong[MONTO],
        ..d.payment.installments.map(item => (item.number, item.due_date, item.amount)).flatten(),
      )
    }
    if d.detraction != none {
      v(1mm)
      strong[DETRACCIÓN: ]
      (
        if present(d.detraction.percent) { "Porcentaje " + d.detraction.percent },
        if present(d.detraction.amount) { "Monto " + d.detraction.amount },
        if present(d.detraction.account) { "Cuenta Banco de la Nación " + d.detraction.account },
      ).filter(present).join(" · ")
    }
  },
  table(
    columns: (auto, auto),
    align: (left, right),
    stroke: none,
    inset: (x: 2.5mm, y: 1.3mm),
    ..d.totals.map(row => (row.label, row.value)).flatten(),
    table.cell(fill: accent, text(fill: white, weight: "bold", d.total.label)),
    table.cell(fill: accent, text(fill: white, weight: "bold", d.total.value)),
  ),
)

#v(3mm)

// QR code, hash and legal text.
#grid(
  columns: (auto, 1fr),
  column-gutter: 4mm,
  align: horizon,
  if present(d.qr) { image(d.qr, width: if small { 2cm } else { 2.6cm }) },
  text(size: base - 1pt, fill: muted, {
    d.legal
    if present(d.hash) {
      linebreak()
      [Resumen: #d.hash]
    }
    if present(d.footer) {
      linebreak()
      d.footer
    }
  }),
)
