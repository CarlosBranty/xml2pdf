// xml2pdf web app: converts SUNAT XML documents to PDF inside the browser.
// The conversion runs in WebAssembly; no file is sent to a server.

import init, { convert, builtinTemplate, defaultSettings } from "./pkg/xml2pdf_wasm.js";

const STORAGE_KEY = "xml2pdf.state.v1";
const RENDER_DELAY_MS = 350;

const $ = (id) => document.getElementById(id);
const elements = {
  dropzone: $("dropzone"),
  fileInput: $("file-input"),
  results: $("results"),
  downloadAll: $("download-all"),
  clear: $("clear"),
  format: $("format"),
  accent: $("accent"),
  logo: $("logo"),
  logoRemove: $("logo-remove"),
  issuerLines: $("issuer-lines"),
  footer: $("footer"),
  showQr: $("show-qr"),
  useTemplate: $("use-template"),
  template: $("template"),
  templateLoad: $("template-load"),
  status: $("status"),
  print: $("print"),
  preview: $("preview"),
  empty: $("empty"),
};

const state = {
  /** Input files: `{ name, bytes }`. */
  inputs: [],
  /** Conversion results: `{ name, url }` or `{ name, error }`. */
  results: [],
  selected: 0,
  settings: {},
  /** Logo image as a `Uint8Array`, or `null`. */
  logo: null,
  template: "",
  useTemplate: false,
};

let renderTimer = 0;

function setStatus(message, isError = false) {
  elements.status.textContent = message;
  elements.status.classList.toggle("error", isError);
}

// --- Persistence -----------------------------------------------------------

const toBase64 = (bytes) => btoa(Array.from(bytes, (byte) => String.fromCharCode(byte)).join(""));
const fromBase64 = (text) => Uint8Array.from(atob(text), (char) => char.charCodeAt(0));

function saveState() {
  try {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        settings: state.settings,
        logo: state.logo ? toBase64(state.logo) : null,
        template: state.template,
        useTemplate: state.useTemplate,
      }),
    );
  } catch {
    // Storage can be unavailable (private mode) or full; settings then last for the session only.
  }
}

function loadState() {
  state.settings = JSON.parse(defaultSettings());
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null");
    if (!saved) return;
    // Only known keys are restored, so an old saved state cannot break the settings.
    for (const key of Object.keys(state.settings)) {
      if (key in (saved.settings ?? {})) state.settings[key] = saved.settings[key];
    }
    state.logo = saved.logo ? fromBase64(saved.logo) : null;
    state.template = saved.template ?? "";
    state.useTemplate = Boolean(saved.useTemplate);
  } catch {
    // A corrupted saved state falls back to the defaults.
  }
}

function fillForm() {
  elements.format.value = state.settings.format;
  elements.accent.value = state.settings.accent_color;
  elements.issuerLines.value = state.settings.issuer_lines.join("\n");
  elements.footer.value = state.settings.footer ?? "";
  elements.showQr.checked = state.settings.show_qr;
  elements.useTemplate.checked = state.useTemplate;
  elements.template.value = state.template;
  elements.logoRemove.hidden = !state.logo;
}

function readForm() {
  state.settings.format = elements.format.value;
  state.settings.accent_color = elements.accent.value;
  state.settings.issuer_lines = elements.issuerLines.value
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  state.settings.footer = elements.footer.value.trim() || null;
  state.settings.show_qr = elements.showQr.checked;
  state.useTemplate = elements.useTemplate.checked;
  state.template = elements.template.value;
}

// --- Conversion ------------------------------------------------------------

function releaseResults() {
  for (const result of state.results) {
    if (result.url) URL.revokeObjectURL(result.url);
  }
  state.results = [];
}

function renderAll() {
  releaseResults();
  const settings = JSON.stringify(state.settings);
  const template = state.useTemplate ? state.template : undefined;

  for (const input of state.inputs) {
    try {
      const files = convert(input.name, input.bytes, settings, state.logo ?? undefined, template);
      for (const file of files) {
        const blob = new Blob([file.pdf], { type: "application/pdf" });
        state.results.push({ name: file.name, url: URL.createObjectURL(blob) });
        file.free();
      }
    } catch (error) {
      state.results.push({ name: input.name, error: String(error.message ?? error) });
    }
  }

  const firstPdf = state.results.findIndex((result) => result.url);
  if (!state.results[state.selected]?.url) state.selected = Math.max(firstPdf, 0);
  showResults();
}

function scheduleRender() {
  readForm();
  saveState();
  clearTimeout(renderTimer);
  renderTimer = setTimeout(renderAll, RENDER_DELAY_MS);
}

function showResults() {
  elements.results.replaceChildren(
    ...state.results.map((result, index) => {
      const item = document.createElement("li");
      if (result.error) {
        item.className = "error";
        item.textContent = `${result.name}: ${result.error}`;
        return item;
      }
      item.classList.toggle("selected", index === state.selected);

      const name = document.createElement("button");
      name.type = "button";
      name.className = "name";
      name.textContent = result.name;
      name.title = "Ver vista previa";
      name.addEventListener("click", () => {
        state.selected = index;
        showResults();
      });

      const download = document.createElement("a");
      download.href = result.url;
      download.download = result.name;
      download.textContent = "Descargar";

      item.append(name, download);
      return item;
    }),
  );

  const pdfs = state.results.filter((result) => result.url);
  const failures = state.results.length - pdfs.length;
  const current = state.results[state.selected];

  elements.downloadAll.disabled = pdfs.length === 0;
  elements.clear.disabled = state.inputs.length === 0;
  elements.print.disabled = !current?.url;
  elements.preview.hidden = !current?.url;
  elements.empty.hidden = Boolean(current?.url);
  if (current?.url) {
    elements.preview.src = current.url;
  } else {
    elements.preview.removeAttribute("src");
  }

  if (state.inputs.length === 0) {
    setStatus("Listo. Agrega un XML o ZIP.");
  } else if (failures > 0) {
    setStatus(`${pdfs.length} PDF generados, ${failures} con error.`, true);
  } else {
    setStatus(`${pdfs.length} PDF generados.`);
  }
}

async function addFiles(fileList) {
  const files = Array.from(fileList);
  if (files.length === 0) return;
  for (const file of files) {
    state.inputs.push({ name: file.name, bytes: new Uint8Array(await file.arrayBuffer()) });
  }
  state.selected = state.results.length;
  renderAll();
}

// --- Events ----------------------------------------------------------------

function bindEvents() {
  elements.fileInput.addEventListener("change", async () => {
    await addFiles(elements.fileInput.files);
    elements.fileInput.value = "";
  });

  for (const type of ["dragenter", "dragover"]) {
    elements.dropzone.addEventListener(type, (event) => {
      event.preventDefault();
      elements.dropzone.classList.add("dragging");
    });
  }
  for (const type of ["dragleave", "drop"]) {
    elements.dropzone.addEventListener(type, (event) => {
      event.preventDefault();
      elements.dropzone.classList.remove("dragging");
    });
  }
  elements.dropzone.addEventListener("drop", (event) => addFiles(event.dataTransfer.files));

  const inputs = [elements.format, elements.accent, elements.issuerLines, elements.footer];
  for (const input of [...inputs, elements.showQr, elements.useTemplate, elements.template]) {
    input.addEventListener("input", scheduleRender);
  }

  elements.logo.addEventListener("change", async () => {
    const [file] = elements.logo.files;
    if (!file) return;
    state.logo = new Uint8Array(await file.arrayBuffer());
    elements.logoRemove.hidden = false;
    scheduleRender();
  });
  elements.logoRemove.addEventListener("click", () => {
    state.logo = null;
    elements.logo.value = "";
    elements.logoRemove.hidden = true;
    scheduleRender();
  });

  elements.templateLoad.addEventListener("click", () => {
    elements.template.value = builtinTemplate(elements.format.value);
    scheduleRender();
  });

  elements.downloadAll.addEventListener("click", () => {
    for (const link of elements.results.querySelectorAll("a[download]")) link.click();
  });
  elements.clear.addEventListener("click", () => {
    state.inputs = [];
    state.selected = 0;
    renderAll();
  });
  elements.print.addEventListener("click", () => {
    elements.preview.contentWindow?.focus();
    elements.preview.contentWindow?.print();
  });
}

async function main() {
  try {
    await init();
  } catch (error) {
    setStatus(`No se pudo cargar el motor de conversión: ${error.message ?? error}`, true);
    return;
  }
  loadState();
  fillForm();
  bindEvents();
  showResults();
}

main();
