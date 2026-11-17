// Smoke test of the web build: loads the WebAssembly module under Node and
// converts the fixtures, as the browser would.
//
// Usage: node scripts/smoke-web.mjs <dist/web> <fixtures>

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const [dist, fixtures] = process.argv.slice(2).map((path) => resolve(path));
assert.ok(dist && fixtures, "usage: smoke-web.mjs <dist/web> <fixtures>");

const wasm = await import(pathToFileURL(`${dist}/pkg/xml2pdf_wasm.js`));
wasm.initSync({ module: readFileSync(`${dist}/pkg/xml2pdf_wasm_bg.wasm`) });

const defaults = JSON.parse(wasm.defaultSettings());
const fixture = (name) => readFileSync(`${fixtures}/${name}`);

function convert(name, overrides = {}, template) {
  const settings = JSON.stringify({ ...defaults, ...overrides });
  return wasm.convert(name, fixture(name), settings, undefined, template).map((file) => {
    const result = {
      name: file.name,
      header: String.fromCharCode(...file.pdf.slice(0, 5)),
      preview: file.preview,
      width: Math.round(file.widthMm),
      height: Math.round(file.heightMm),
    };
    file.free();
    return result;
  });
}

const cases = [
  ["factura.xml", "a4", ["20123456789-01-F001-00000123.pdf"]],
  ["boleta.xml", "ticket80", ["20123456789-03-B001-00004567.pdf"]],
  ["nota-credito.xml", "a5", ["20123456789-07-FC01-00000008.pdf"]],
  ["nota-debito.xml", "ticket58", ["20123456789-08-FD01-00000003.pdf"]],
  ["documentos.zip", "a4", ["20123456789-01-F001-00000123.pdf", "20123456789-07-FC01-00000008.pdf"]],
];

for (const [name, format, expected] of cases) {
  const files = convert(name, { format });
  assert.deepEqual(files.map((file) => file.name), expected, `${name}: file names`);
  for (const file of files) {
    assert.equal(file.header, "%PDF-", `${name}: PDF header`);
    assert.ok(file.preview.startsWith("<svg"), `${name}: SVG preview`);
  }
  console.log(`ok ${name} (${format}) -> ${expected.join(", ")}`);
}

const [a4] = convert("factura.xml", { format: "a4" });
assert.deepEqual([a4.width, a4.height], [210, 297], "the page size is reported");
const [ticket] = convert("factura.xml", { format: "ticket80" });
assert.ok(ticket.width === 80 && ticket.height > 60, "tickets report their real height");

assert.throws(() => convert("factura.xml", { format: "legal" }), /invalid settings/);
assert.throws(() => convert("factura.xml", { accent_color: "#aa0000" }), /invalid settings/);
assert.throws(() => convert("factura.xml", {}, "#let broken = ("), /template failed to compile/);
assert.throws(() => wasm.builtinTemplate("legal"), /unknown format/);
assert.match(wasm.builtinTemplate("ticket58"), /data\.json/);
assert.match(wasm.templateData("factura.xml", fixture("factura.xml"), JSON.stringify(defaults)), /F001-00000123/);

// Bulk conversion: split a ZIP, summarize each document and pack the results.
const entries = wasm.extract("documentos.zip", fixture("documentos.zip"));
assert.deepEqual(entries.map((entry) => entry.name), ["factura.xml", "nota-credito.xml"]);
assert.deepEqual(JSON.parse(wasm.summary(entries[0].bytes)), {
  file_stem: "20123456789-01-F001-00000123",
  id: "F001-00000123",
  title: "FACTURA ELECTRÓNICA",
  issue_date: "15/01/2026",
  customer: "CONSTRUCTORA DEL SUR E.I.R.L.",
  total: "S/ 1,593.00",
});
assert.throws(() => wasm.summary(new TextEncoder().encode("<ApplicationResponse/>")), /unsupported document/);

const batchPdf = wasm.renderPdf(entries[1].bytes, JSON.stringify({ ...defaults, format: "ticket80" }), undefined, undefined);
assert.equal(String.fromCharCode(...batchPdf.slice(0, 5)), "%PDF-", "renderPdf returns a PDF");

const bundle = new wasm.ZipBundle();
for (const entry of entries) bundle.add(entry.name, entry.bytes);
bundle.add(entries[0].name, entries[0].bytes);
const packed = wasm.extract("lote.zip", bundle.finish());
assert.deepEqual(
  packed.map((entry) => entry.name),
  ["factura.xml", "nota-credito.xml", "factura (2).xml"],
  "packed files keep their order and repeated names get a suffix",
);
assert.deepEqual(packed[1].bytes, entries[1].bytes);

// Format designer: catalog, presets and the preview with block positions.
const catalog = JSON.parse(wasm.designCatalog());
assert.deepEqual(catalog.presets.map((preset) => preset.id), ["clasico", "moderno", "compacto", "minimo"]);
assert.equal(catalog.blocks.length, 15);
const modern = JSON.parse(wasm.presetDesign("moderno", "page"));
assert.equal(modern.style.heading, "inverted");
assert.throws(() => wasm.presetDesign("barroco", "page"), /unknown variant/);

const designed = wasm.designPreview(fixture("factura.xml"), JSON.stringify({ ...defaults, page_design: modern }), undefined);
assert.ok(designed.svg.startsWith("<svg"));
assert.equal(Math.round(designed.widthPt), 595);
const placed = JSON.parse(designed.blocks);
assert.deepEqual(placed.slice(0, 3).map((block) => block.id), ["r0b0", "r0b1", "r1b0"]);
assert.ok(placed.every((block) => block.width > 0 && block.height > 0));
assert.throws(
  () => wasm.designPreview(fixture("factura.xml"), JSON.stringify({ ...defaults, page_design: { rows: [{ blocks: [] }] } }), undefined),
  /invalid design/,
);
const [letter] = convert("factura.xml", { format: "letter" });
assert.deepEqual([letter.width, letter.height], [216, 279], "letter paper is offered");

console.log("web smoke test passed");
