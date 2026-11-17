// xml2pdf built-in template for thermal tickets (80 mm and 58 mm).
//
// Copy this file to start a custom template. Everything printed comes from
// `data.json`; its fields are described in docs/templates.md.

#let d = json("data.json")
#let narrow = d.page.width < 70
#let base = if narrow { 6.5pt } else { 8pt }

#set document(title: d.doc.title + " " + d.doc.id, author: d.issuer.name)
#set page(
  width: d.page.width * 1mm,
  height: auto,
  margin: (x: if narrow { 2.5mm } else { 4mm }, y: 4mm),
)
#set text(font: "Noto Sans", size: base, lang: "es")
#set par(leading: 0.45em, spacing: 0.6em)

#let strong(body) = text(weight: "bold", body)
#let present(value) = value != none and value != ""
#let rule() = line(length: 100%, stroke: (thickness: 0.5pt, dash: "dashed"))
#let pair(label, value) = grid(
  columns: (1fr, auto),
  column-gutter: 2mm,
  label, align(right, value),
)

// Issuer.
#align(center, {
  if present(d.logo) {
    image(d.logo, width: if narrow { 55% } else { 45% })
  }
  text(size: base + 1.5pt, weight: "bold", d.issuer.name)
  linebreak()
  [RUC #d.issuer.ruc]
  let details = (d.issuer.trade_name, d.issuer.address, ..d.issuer.extra).filter(present)
  if details.len() > 0 {
    linebreak()
    details.join(linebreak())
  }
})

#rule()

#align(center, {
  strong(d.doc.title)
  linebreak()
  text(size: base + 1.5pt, weight: "bold", d.doc.id)
})

#rule()

#for item in d.doc.fields + d.customer.fields [
  #strong(item.label + ":") #item.value \
]

#if d.affected != none [
  #strong[MODIFICA:] #d.affected.document \
  #strong[MOTIVO:] #(d.affected.reason, d.affected.description).filter(present).join(" - ")
]

#rule()

// Items: description on one row, quantity and amounts below it.
#pair(strong[DESCRIPCIÓN], strong[TOTAL])
#for line in d.lines {
  block(spacing: 1.2mm, {
    line.description
    pair([#line.quantity #line.unit x #line.unit_price], line.total)
  })
}

#rule()

#for row in d.totals {
  pair(row.label, row.value)
}
#text(size: base + 1.5pt, pair(strong(d.total.label), strong(d.total.value)))

#v(1mm)
SON: #d.amount_in_words

#for legend in d.legends [
  #legend
]

#if d.notes.len() > 0 [
  #strong[OBS.:] #d.notes.join(linebreak())
]

#if d.payment != none and d.payment.installments.len() > 0 {
  if present(d.payment.pending) {
    pair(strong[PENDIENTE DE PAGO], d.payment.pending)
  }
  for item in d.payment.installments {
    pair([Cuota #item.number · #item.due_date], item.amount)
  }
}

#if d.detraction != none [
  #strong[DETRACCIÓN:] #(d.detraction.percent, d.detraction.amount, d.detraction.account).filter(present).join(" · ")
]

#rule()

#align(center, {
  if present(d.qr) {
    image(d.qr, width: if narrow { 55% } else { 42% })
  }
  text(size: base - 1pt, {
    d.legal
    if present(d.hash) {
      linebreak()
      [Resumen: #d.hash]
    }
    if present(d.footer) {
      linebreak()
      d.footer
    }
  })
})

// Credit of the tool, with its addresses as links.
#v(1mm)
#align(center, text(size: base - 1.5pt, {
  d.credit.text
  linebreak()
  d.credit.links.map(address => link(address)).join(" o ")
}))
