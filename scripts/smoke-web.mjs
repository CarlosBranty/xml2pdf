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
    const result = { name: file.name, header: String.fromCharCode(...file.pdf.slice(0, 5)) };
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
  for (const file of files) assert.equal(file.header, "%PDF-", `${name}: PDF header`);
  console.log(`ok ${name} (${format}) -> ${expected.join(", ")}`);
}

assert.throws(() => convert("factura.xml", { format: "letter" }), /invalid settings/);
assert.throws(() => convert("factura.xml", { accent_color: "blue" }), /invalid accent color/);
assert.throws(() => convert("factura.xml", {}, "#let broken = ("), /template failed to compile/);
assert.throws(() => wasm.builtinTemplate("letter"), /unknown format/);
assert.match(wasm.builtinTemplate("ticket58"), /data\.json/);
assert.match(wasm.templateData("factura.xml", fixture("factura.xml"), JSON.stringify(defaults)), /F001-00000123/);

console.log("web smoke test passed");
