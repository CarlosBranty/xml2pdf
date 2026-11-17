// xml2pdf built-in template.
//
// It draws a design: rows of blocks (issuer, customer, items, totals...) with
// a few style options, for pages and for thermal tickets. Black and gray only.
//
// Copy this file to start a custom template. Everything printed comes from
// `data.json`; its fields are described in docs/templates.md.

#let d = json("data.json")
#let style = d.design.style
#let ticket = d.page.ticket
#let narrow = ticket and d.page.width < 70
#let small = d.format == "a5"

#let scale = (small: 0.88, normal: 1, large: 1.12).at(style.text)
#let base = (if narrow or small { 6.5pt } else { 8pt }) * scale
#let border = 0.6pt + luma(110)
#let shade = luma(232)
#let stripe = luma(240)
#let radius = if style.boxes == "rounded" { 4pt } else { 0pt }
#let inset = if small { 2mm } else { 3mm }
#let gap = 1.5mm
// Width of the totals when they have a row to themselves.
#let side = if small { 4.8cm } else { 7cm }
// A short items table is padded with empty rows up to this many.
#let min-rows = if small { 6 } else { 8 }

#set document(title: d.doc.title + " " + d.doc.id, author: d.issuer.name)
#set page(
  width: d.page.width * 1mm,
  height: auto,
  margin: (x: if narrow { 2.5mm } else { 4mm }, y: 4mm),
) if ticket
#set page(paper: d.page.paper, margin: if small { 8mm } else { 12mm }) if not ticket
#set text(font: "Noto Sans", size: base, lang: "es")
#set par(leading: 0.45em, spacing: 0.6em)
#set block(spacing: if ticket { 0.6em } else { gap })

#let strong(body) = text(weight: "bold", body)
#let present(value) = value != none and value != ""
#let alignment-of(block) = (left: left, center: center, right: right).at(block.align)
// Fill and text color of headings: the document box and the items heading.
#let heading-fill = if style.heading == "shaded" { shade } else if style.heading == "inverted" { luma(30) }
#let heading-text = if style.heading == "inverted" { white } else { black }

// A block drawn inside a box, a line, or nothing, as the design says.
#let panel(body, boxed: true, fill: none, height: auto) = block(
  width: 100%,
  height: height,
  ..if boxed and style.boxes != "none" {
    (
      stroke: if style.boxes == "lines" { (bottom: border) } else { border },
      radius: radius,
      inset: inset,
      fill: fill,
    )
  },
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
// Lines of `LABEL: value` for tickets.
#let lines(items) = items.filter(item => present(item.value)).map(item => [#strong(item.label + ":") #item.value]).join(linebreak())
#let pair(label, value) = grid(
  columns: (1fr, auto),
  column-gutter: 2mm,
  label, align(right, value),
)

#let payment = if d.payment != none { d.payment } else { (form: none, pending: none, installments: ()) }
#let detraction = if d.detraction != none {
  (
    d.detraction.percent,
    d.detraction.amount,
    if present(d.detraction.account) { "Cuenta Banco de la Nación " + d.detraction.account },
  ).filter(present).join(" · ")
}

// Heading, width and alignment of each column the items table can show.
#let item-columns = (
  quantity: ("CANT.", 9%, center),
  unit_code: ("UM", 8%, center),
  unit: ("UM", 8%, center),
  code: ("CÓD.", 10%, center),
  description: ("DESCRIPCIÓN", 1fr, left),
  unit_value: ("V/U", 12%, right),
  unit_price: ("P/U", 12%, right),
  discount: ("DSCTO.", 10%, right),
  value: ("V. VENTA", 13%, right),
  tax: ("IGV", 10%, right),
  total: ("IMPORTE", 13%, right),
)

// --- Blocks -----------------------------------------------------------------

#let issuer(b) = align(alignment-of(b), {
  if present(d.logo) {
    if ticket {
      image(d.logo, width: if narrow { 55% } else { 45% })
    } else {
      image(d.logo, height: if small { 1.2cm } else { 1.8cm })
      v(1mm)
    }
  }
  text(size: if ticket { base + 1.5pt } else { base }, weight: "bold", d.issuer.name)
  if ticket {
    linebreak()
    [RUC #d.issuer.ruc]
  }
  let details = (d.issuer.trade_name, d.issuer.address, ..d.issuer.extra).filter(present)
  if details.len() > 0 {
    linebreak()
    text(size: if ticket { base } else { base - 0.5pt }, details.join(linebreak()))
  }
})

#let document-box(b, height: auto) = if ticket {
  align(alignment-of(b), {
    strong(d.doc.title)
    linebreak()
    text(size: base + 1.5pt, weight: "bold", d.doc.id)
  })
} else {
  panel(boxed: b.boxed, fill: if b.boxed { heading-fill }, height: height, align(alignment-of(b) + horizon, {
    set par(leading: 0.5em)
    text(size: if small { base + 4pt } else { base + 8pt }, weight: "bold", fill: if b.boxed { heading-text } else { black }, {
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
  }))
}

#let customer(b, height: auto) = if ticket {
  lines(d.customer.fields)
} else {
  panel(boxed: b.boxed, height: height, {
    strong[DATOS DEL CLIENTE]
    v(0.3mm)
    rows((
      (d.customer.doc_label, d.customer.doc_number),
      ("DENOMINACIÓN", d.customer.name),
      ("DIRECCIÓN", d.customer.address),
    ))
  })
}

#let dates(b, height: auto) = if ticket {
  lines(d.doc.fields)
} else {
  panel(boxed: b.boxed, height: height, rows((
    ("FECHA EMISIÓN", d.doc.issue_date),
    ("FECHA DE VENC.", d.doc.due_date),
    ("MONEDA", d.doc.currency_name),
  )))
}

#let affected(b, height: auto) = {
  let reason = (d.affected.reason, d.affected.description).filter(present).join(" - ")
  if ticket [
    #strong[MODIFICA:] #d.affected.document \
    #strong[MOTIVO:] #reason
  ] else {
    panel(boxed: b.boxed, height: height, rows((
      ("DOCUMENTO QUE MODIFICA", d.affected.document),
      ("MOTIVO", reason),
    )))
  }
}

#let items(b) = if ticket {
  // Description on one row, quantity and amounts below it.
  pair(strong[DESCRIPCIÓN], strong[TOTAL])
  for line in d.lines {
    block(spacing: 1.2mm, {
      line.description
      pair([#line.quantity #line.unit x #line.unit_price], line.total)
    })
  }
} else {
  let shown = b.columns.map(name => item-columns.at(name))
  let widths = shown.map(column => column.at(1))
  let aligns = shown.map(column => column.at(2))
  // The heading is a separate box so it can have rounded corners; both use
  // the same column widths.
  block(
    width: 100%,
    stroke: if style.heading == "plain" { (bottom: border) } else if style.boxes == "none" { none } else { border },
    radius: if style.heading == "plain" { 0pt } else { radius },
    fill: heading-fill,
    grid(
      columns: widths,
      align: aligns,
      inset: (x: 1.5mm, y: 1.7mm),
      ..shown.map(column => text(weight: "bold", fill: heading-text, column.at(0))),
    ),
  )
  let filler = if style.filler_rows { calc.max(0, min-rows - d.lines.len()) } else { 0 }
  table(
    columns: widths,
    align: aligns,
    stroke: (x: none, y: 0.4pt + luma(150)),
    inset: (x: 1.5mm, y: 1.2mm),
    fill: (_, row) => if style.stripes and calc.odd(row) { stripe },
    ..d.lines.map(line => b.columns.map(name => line.at(name))).flatten(),
    ..range(filler * widths.len()).map(_ => [~]),
  )
}

#let totals(b, alone: true) = if ticket {
  for row in d.totals {
    pair(row.label, row.value)
  }
  text(size: base + 1.5pt, pair(strong(d.total.label), strong(d.total.value)))
} else {
  // The currency symbol has its own column.
  let amounts = grid(
    columns: (1fr, auto, if small { 1.8cm } else { 2.4cm }),
    align: (right, center, right),
    column-gutter: if small { 3mm } else { 5mm },
    row-gutter: 1.2mm,
    ..(..d.totals, d.total).map(row => (
      strong(row.label),
      strong(d.doc.currency_symbol),
      strong(row.amount),
    )).flatten(),
  )
  let body = panel(boxed: b.boxed, amounts)
  if alone { align(alignment-of(b), block(width: side, inset: (right: 1.5mm), body)) } else { body }
}

#let words(b, height: auto) = if ticket [
  SON: #d.amount_in_words
] else {
  panel(boxed: b.boxed, height: height, pad(left: if b.boxed { 8% } else { 0% }, [#strong[IMPORTE EN LETRAS] : #d.amount_in_words]))
}

#let details(b) = if ticket {
  (
    ..d.legends,
    if d.notes.len() > 0 [#strong[OBS.:] #d.notes.join(linebreak())],
    if present(payment.pending) { pair(strong[PENDIENTE DE PAGO], payment.pending) },
    ..payment.installments.map(item => pair([Cuota #item.number · #item.due_date], item.amount)),
    if present(detraction) [#strong[DETRACCIÓN:] #detraction],
  ).filter(present).join(parbreak())
} else {
  let entries = (
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
  if entries.len() > 0 {
    panel(boxed: b.boxed, block(inset: (y: 1mm), rows(entries, label-width: 36%, label-align: right)))
  }
}

#let legal(b, height: auto) = {
  let body = {
    d.legal
    if not ticket {
      for legend in d.legends {
        linebreak()
        legend
      }
    }
    if ticket and present(d.hash) {
      linebreak()
      [Resumen: #d.hash]
    }
    if present(d.footer) {
      linebreak()
      d.footer
    }
    if not ticket and present(d.hash) {
      linebreak()
      [#strong[Resumen:] #d.hash]
    }
  }
  if ticket {
    align(alignment-of(b), text(size: base - 1pt, body))
  } else {
    panel(boxed: b.boxed, height: height, align(alignment-of(b), body))
  }
}

#let qr(b, alone: true, height: auto) = if ticket {
  align(alignment-of(b), image(d.qr, width: if narrow { 55% } else { 42% }))
} else if alone {
  align(alignment-of(b), image(d.qr, width: if small { 2.4cm } else { 3cm }))
} else {
  panel(boxed: b.boxed, height: height, align(alignment-of(b) + horizon, image(d.qr, width: 100%)))
}

#let credit(b) = {
  if ticket { v(1mm) }
  align(alignment-of(b), text(size: base - 1.5pt, fill: if ticket { black } else { luma(90) }, {
    d.credit.text
    if ticket { linebreak() } else [ ]
    d.credit.links.map(address => link(address)).join(" o ")
  }))
}

#let free-text(b, height: auto) = panel(boxed: b.boxed, height: height, align(alignment-of(b), b.text))

#let rule(b) = line(
  length: 100%,
  stroke: if ticket { (thickness: 0.5pt, dash: "dashed") } else { 0.5pt + luma(110) },
)

// Blocks that have nothing to show for this document are left out.
#let visible(b) = (
  (b.type != "affected" or d.affected != none)
    and (b.type != "qr" or present(d.qr))
    and (b.type != "text" or present(b.text))
)

// Draws one block. `alone` tells whether it has the row to itself, and
// `height` is the shared height of a row of boxes.
#let draw(b, alone: true, height: auto) = {
  let kind = b.type
  if kind == "issuer" {
    issuer(b)
  } else if kind == "document" {
    document-box(b, height: height)
  } else if kind == "customer" {
    customer(b, height: height)
  } else if kind == "dates" {
    dates(b, height: height)
  } else if kind == "affected" {
    affected(b, height: height)
  } else if kind == "items" {
    items(b)
  } else if kind == "totals" {
    totals(b, alone: alone)
  } else if kind == "words" {
    words(b, height: height)
  } else if kind == "details" {
    details(b)
  } else if kind == "legal" {
    legal(b, height: height)
  } else if kind == "qr" {
    qr(b, alone: alone, height: height)
  } else if kind == "credit" {
    credit(b)
  } else if kind == "text" {
    free-text(b, height: height)
  } else if kind == "rule" {
    rule(b)
  } else if kind == "spacer" {
    v(if ticket { 2mm } else { 4mm })
  }
}

// In the editor, each block records its place so the interface can put a
// handle over it. The mark takes no room, so the layout does not change.
#let marked(b, width, body) = if d.editor {
  let height = measure(width: width, body).height
  block(width: 100%, {
    place(top + left, [#metadata((id: b.id, w: width.pt(), h: height.pt())) <block>])
    body
  })
} else {
  body
}

#for row in d.design.rows {
  let blocks = row.blocks.filter(visible)
  if blocks.len() == 1 {
    let b = blocks.first()
    // Each block starts on its own line. Outside the editor it is placed
    // directly, so a long items table can continue on the next page.
    let body = draw(b)
    if body != none {
      if d.editor { layout(size => marked(b, size.width, body)) } else { block(width: 100%, body) }
    }
  } else if blocks.len() > 1 {
    layout(size => {
      let shares = blocks.map(b => b.width).sum()
      let free = size.width - gap * (blocks.len() - 1)
      let widths = blocks.map(b => free * b.width / shares)
      // Boxes of a row share the height of the tallest block.
      let height = calc.max(..blocks.zip(widths).map(((b, width)) => measure(
        width: width,
        draw(b, alone: false),
      ).height))
      grid(
        columns: widths,
        column-gutter: gap,
        ..blocks.zip(widths).map(((b, width)) => marked(b, width, draw(b, alone: false, height: height))),
      )
    })
  }
}
