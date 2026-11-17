// xml2pdf web app: converts SUNAT XML documents to PDF inside the browser.
// The conversion runs in WebAssembly; no file is sent to a server.

import init, {
  ZipBundle,
  builtinTemplate,
  convert,
  defaultSettings,
  extract,
  renderPdf,
  summary,
} from "./pkg/xml2pdf_wasm.js";
import * as platform from "./platform.js";

const STORAGE_KEY = "xml2pdf.state.v2";
const RENDER_DELAY_MS = 350;
const PASTED_NAME = "pegado.xml";

// Batch limits, so a large selection cannot exhaust the browser's memory.
const MAX_DOCUMENTS = 100;
const MAX_TOTAL_BYTES = 50 * 1024 * 1024;
/** File chips shown before the rest collapse into a counter. */
const CHIPS_SHOWN = 6;

/** Formats shown after converting, in display order. */
const FORMATS = [
  { id: "a4", title: "A4", size: "210 × 297 mm", family: "page" },
  { id: "a5", title: "A5", size: "148 × 210 mm", family: "page" },
  { id: "ticket80", title: "Ticket 80", size: "80 mm", family: "ticket" },
  { id: "ticket58", title: "Ticket 58", size: "58 mm", family: "ticket" },
];

const PDF_TYPE = { description: "PDF", mime: "application/pdf", extension: ".pdf" };
const ZIP_TYPE = { description: "ZIP", mime: "application/zip", extension: ".zip" };

const $ = (id) => document.getElementById(id);

const state = {
  /** Files chosen or dropped: `{ name, bytes }`. */
  files: [],
  /**
   * Documents of the batch, one per XML: `{ name, bytes, summary }` when it
   * can be read, `{ name, error }` when it cannot.
   */
  documents: [],
  /** Documents left out because the batch was over the limit. */
  omitted: 0,
  selected: 0,
  /** Rendered formats of the selected document: `{ [formatId]: { fileName, blob, pdfUrl, previewUrl } }`. */
  current: null,
  /** Why the selected document could not be rendered, if it failed. */
  renderError: "",
  settings: {},
  /** Logo image as a `Uint8Array`, or `null`. */
  logo: null,
  template: "",
  templateTarget: "page",
  useTemplate: false,
};

let ready = false;
let saving = false;
let renderTimer = 0;

function setStatus(message, isError = false) {
  $("status").textContent = message;
  $("status").classList.toggle("error", isError);
}

const errorText = (error) => String(error?.message ?? error);

// --- Saved options ---------------------------------------------------------

const toBase64 = (bytes) => btoa(Array.from(bytes, (byte) => String.fromCharCode(byte)).join(""));
const fromBase64 = (text) => Uint8Array.from(atob(text), (char) => char.charCodeAt(0));

function saveOptions() {
  try {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        settings: state.settings,
        logo: state.logo ? toBase64(state.logo) : null,
        template: state.template,
        templateTarget: state.templateTarget,
        useTemplate: state.useTemplate,
      }),
    );
  } catch {
    // Storage can be unavailable (private mode) or full; options then last for the session only.
  }
}

function loadOptions() {
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
    state.templateTarget = saved.templateTarget === "ticket" ? "ticket" : "page";
    state.useTemplate = Boolean(saved.useTemplate);
  } catch {
    // A corrupted saved state falls back to the defaults.
  }
}

function fillOptions() {
  $("issuer-lines").value = state.settings.issuer_lines.join("\n");
  $("footer").value = state.settings.footer ?? "";
  $("show-qr").checked = state.settings.show_qr;
  $("use-template").checked = state.useTemplate;
  $("template-target").value = state.templateTarget;
  $("template").value = state.template;
  $("logo-remove").hidden = !state.logo;
}

function readOptions() {
  state.settings.issuer_lines = $("issuer-lines")
    .value.split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  state.settings.footer = $("footer").value.trim() || null;
  state.settings.show_qr = $("show-qr").checked;
  state.useTemplate = $("use-template").checked;
  state.templateTarget = $("template-target").value;
  state.template = $("template").value;
}

/** The options as the platform layer receives them. */
function currentOptions() {
  return {
    settings: state.settings,
    logo: state.logo,
    template: state.template,
    templateTarget: state.templateTarget,
    useTemplate: state.useTemplate,
  };
}

/** Arguments shared by every render of a format: settings, logo and template. */
function renderArguments(format) {
  const settings = JSON.stringify({ ...state.settings, format: format.id });
  const template =
    state.useTemplate && state.templateTarget === format.family ? state.template : undefined;
  return [settings, state.logo ?? undefined, template];
}

// --- Input -----------------------------------------------------------------

/** Everything to convert: the chosen files plus the pasted text, if any. */
function inputs() {
  const pasted = $("xml-text").value.trim();
  const text = pasted ? [{ name: PASTED_NAME, bytes: new TextEncoder().encode(pasted) }] : [];
  return [...state.files, ...text];
}

function showInput() {
  const chips = state.files.slice(0, CHIPS_SHOWN).map((file, index) => {
    const item = document.createElement("li");
    const name = document.createElement("span");
    name.textContent = file.name;
    const remove = document.createElement("button");
    remove.type = "button";
    remove.textContent = "×";
    remove.setAttribute("aria-label", `Quitar ${file.name}`);
    remove.addEventListener("click", () => {
      state.files.splice(index, 1);
      showInput();
    });
    item.append(name, remove);
    return item;
  });

  if (state.files.length > CHIPS_SHOWN) {
    const more = document.createElement("li");
    more.className = "more";
    more.textContent = `+${state.files.length - CHIPS_SHOWN} más`;
    chips.push(more);
  }
  if (state.files.length > 1) {
    const item = document.createElement("li");
    const clear = document.createElement("button");
    clear.type = "button";
    clear.className = "clear";
    clear.textContent = "Quitar todos";
    clear.addEventListener("click", () => {
      state.files = [];
      showInput();
    });
    item.append(clear);
    chips.push(item);
  }

  $("files").replaceChildren(...chips);
  $("convert").disabled = !ready || inputs().length === 0;
}

/** Adds files up to the batch limits and reports how many were left out. */
async function addFiles(fileList) {
  let bytes = state.files.reduce((total, file) => total + file.bytes.length, 0);
  let skipped = 0;
  for (const file of Array.from(fileList)) {
    if (state.files.length >= MAX_DOCUMENTS || bytes + file.size > MAX_TOTAL_BYTES) {
      skipped += 1;
      continue;
    }
    bytes += file.size;
    state.files.push({ name: file.name, bytes: new Uint8Array(await file.arrayBuffer()) });
  }
  showInput();

  if (skipped > 0) {
    setStatus(
      `Se agregaron ${state.files.length} archivos; ${skipped} quedaron fuera por el límite de ` +
        `${MAX_DOCUMENTS} comprobantes o ${MAX_TOTAL_BYTES / 1024 / 1024} MB por tanda.`,
      true,
    );
  } else if (state.files.length > 0) {
    setStatus(
      state.files.length === 1
        ? "Archivo listo. Presiona Convertir."
        : `${state.files.length} archivos listos. Presiona Convertir.`,
    );
  }
}

// --- Conversion ------------------------------------------------------------

function releaseCurrent() {
  for (const file of Object.values(state.current ?? {})) {
    URL.revokeObjectURL(file.pdfUrl);
    URL.revokeObjectURL(file.previewUrl);
  }
  state.current = null;
  state.renderError = "";
}

/**
 * Reads every input into the list of documents of the batch. ZIP files
 * contribute one document per XML inside them. Nothing is rendered here, so
 * a large batch stays cheap.
 */
function readDocuments() {
  state.documents = [];
  state.omitted = 0;

  for (const input of inputs()) {
    let entries;
    try {
      entries = extract(input.name, input.bytes);
    } catch (error) {
      state.documents.push({ name: input.name, error: errorText(error) });
      continue;
    }
    for (const entry of entries) {
      if (state.documents.length >= MAX_DOCUMENTS) {
        state.omitted += 1;
      } else {
        const item = { name: entry.name, bytes: entry.bytes };
        try {
          item.summary = JSON.parse(summary(item.bytes));
        } catch (error) {
          item.error = errorText(error);
        }
        state.documents.push(item);
      }
      entry.free();
    }
  }
}

/** Renders the selected document in every format, with previews. */
function renderSelected() {
  releaseCurrent();
  const selected = state.documents[state.selected];
  if (!selected?.summary) return;

  const rendered = {};
  try {
    for (const format of FORMATS) {
      const [file] = convert(selected.name, selected.bytes, ...renderArguments(format));
      const blob = new Blob([file.pdf], { type: PDF_TYPE.mime });
      const preview = new Blob([file.preview], { type: "image/svg+xml" });
      rendered[format.id] = {
        fileName: `${selected.summary.file_stem}-${format.id}.pdf`,
        formatId: format.id,
        pageMm: [file.widthMm, file.heightMm],
        blob,
        pdfUrl: URL.createObjectURL(blob),
        previewUrl: URL.createObjectURL(preview),
      };
      file.free();
    }
    state.current = rendered;
  } catch (error) {
    state.current = rendered;
    releaseCurrent();
    state.renderError = errorText(error);
  }
}

function convertAll() {
  if (inputs().length === 0) {
    setStatus("Pega, arrastra o elige un XML primero.", true);
    return;
  }
  readDocuments();
  const firstValid = state.documents.findIndex((item) => item.summary);
  state.selected = Math.max(firstValid, 0);
  renderSelected();
  showResults();
}

/** Renders again after an option changes, when there is something on screen. */
function scheduleRender() {
  readOptions();
  saveOptions();
  platform.optionsChanged(currentOptions());
  if ($("resultado").hidden) return;
  clearTimeout(renderTimer);
  renderTimer = setTimeout(() => {
    renderSelected();
    showResults();
  }, RENDER_DELAY_MS);
}

// --- Results ---------------------------------------------------------------

function documentRow(item, index) {
  const row = document.createElement("li");
  const cells = (parent, values, lastClass) => {
    values.forEach((value, position) => {
      const cell = document.createElement(position === 0 ? "b" : "span");
      cell.textContent = value;
      if (lastClass && position === values.length - 1) cell.className = lastClass;
      parent.append(cell);
    });
  };

  if (item.error) {
    const failed = document.createElement("div");
    failed.className = "failed";
    cells(failed, [item.name, item.error]);
    row.append(failed);
    return row;
  }

  const button = document.createElement("button");
  button.type = "button";
  button.setAttribute("aria-current", String(index === state.selected));
  cells(button, [item.summary.id, item.summary.title, item.summary.customer, item.summary.total], "total");
  button.addEventListener("click", () => {
    state.selected = index;
    renderSelected();
    showResults();
  });
  row.append(button);
  return row;
}

function formatCard(format, file) {
  const card = document.createElement("article");
  card.className = "format";

  const header = document.createElement("header");
  const title = document.createElement("h3");
  title.textContent = format.title;
  const size = document.createElement("span");
  size.textContent = format.size;
  header.append(title, size);

  const sheet = document.createElement("a");
  sheet.className = "sheet";
  sheet.href = file.pdfUrl;
  sheet.target = "_blank";
  sheet.rel = "noopener";
  sheet.title = "Abrir el PDF en otra pestaña";
  const image = document.createElement("img");
  image.src = file.previewUrl;
  image.alt = `Vista previa en ${format.title}`;
  sheet.append(image);

  const actions = document.createElement("div");
  actions.className = "format-actions";
  const save = document.createElement("button");
  save.type = "button";
  save.className = "button save";
  save.textContent = "Guardar";
  save.addEventListener("click", () => report(platform.save(file.blob, file.fileName, PDF_TYPE)));
  const print = document.createElement("button");
  print.type = "button";
  print.className = "button";
  print.textContent = "Imprimir";
  print.addEventListener("click", () => report(platform.print(file)));
  actions.append(save, print);

  card.append(header, sheet, actions);
  return card;
}

function showResults() {
  const valid = state.documents.filter((item) => item.summary).length;
  const failed = state.documents.length - valid;
  const isBatch = state.documents.length > 1;
  const selected = state.documents[state.selected];

  $("notice").hidden = state.omitted === 0;
  $("notice").textContent =
    `Se convirtieron los primeros ${MAX_DOCUMENTS} comprobantes; ${state.omitted} quedaron fuera ` +
    "de esta tanda. Conviértelos en una tanda aparte.";

  // A lone document that failed has no list, so its error is shown on its own.
  const errors = [];
  if (!isBatch && selected?.error) errors.push(`${selected.name}: ${selected.error}`);
  if (state.renderError) errors.push(state.renderError);
  $("errors").replaceChildren(
    ...errors.map((message) => {
      const item = document.createElement("li");
      item.textContent = message;
      return item;
    }),
  );

  $("batch").hidden = !isBatch;
  $("documents").replaceChildren(...(isBatch ? state.documents.map(documentRow) : []));
  $("save-all").disabled = saving || valid === 0;
  $("results-title").textContent = isBatch ? `${valid} comprobantes` : "Elige tu formato";

  $("formats").replaceChildren(
    ...(state.current ? FORMATS.map((format) => formatCard(format, state.current[format.id])) : []),
  );
  $("save-hint").textContent = platform.saveHint();
  $("save-hint").hidden = !state.current;
  $("resultado").hidden = false;

  if (valid === 0) {
    setStatus("No se pudo convertir. Revisa el detalle más abajo.", true);
  } else if (failed > 0 || state.omitted > 0 || state.renderError) {
    setStatus(`${valid} comprobante(s) listo(s); revisa los avisos más abajo.`, true);
  } else if (isBatch) {
    setStatus(`Listo: ${valid} comprobantes. Elige uno para verlo o guárdalos todos.`);
  } else {
    setStatus("Listo: tu PDF está en los cuatro formatos.");
  }
}

/** Lets the page repaint between two documents of a batch. */
const pause = () => new Promise((resolve) => setTimeout(resolve));

/** Renders the whole batch in one format and saves it as a ZIP file. */
async function saveAll() {
  const format = FORMATS.find((item) => item.id === $("bulk-format").value);
  const batch = state.documents.filter((item) => item.summary);
  if (saving || batch.length === 0) return;

  saving = true;
  $("save-all").disabled = true;
  const bundle = new ZipBundle();
  let failures = 0;
  try {
    for (const [index, item] of batch.entries()) {
      $("bulk-status").textContent = `Generando ${index + 1} de ${batch.length}…`;
      await pause();
      try {
        bundle.add(`${item.summary.file_stem}.pdf`, renderPdf(item.bytes, ...renderArguments(format)));
      } catch {
        failures += 1;
      }
    }
    const done = batch.length - failures;
    if (done === 0) {
      bundle.free();
      $("bulk-status").textContent = "No se pudo generar ningún PDF en ese formato.";
      return;
    }
    const zip = new Blob([bundle.finish()], { type: ZIP_TYPE.mime });
    await report(platform.save(zip, `comprobantes-${format.id}.zip`, ZIP_TYPE));
    $("bulk-status").textContent =
      failures === 0 ? `${done} PDF en el ZIP.` : `${done} PDF en el ZIP; ${failures} con error.`;
  } finally {
    saving = false;
    $("save-all").disabled = false;
  }
}

/** Shows the outcome of a platform action (saving, printing) in the status line. */
async function report(action) {
  try {
    const message = await action;
    if (message) setStatus(message);
  } catch (error) {
    setStatus(errorText(error), true);
  }
}

// --- Events ----------------------------------------------------------------

function bindEvents() {
  const fileInput = $("file-input");
  for (const id of ["pick", "pick-round"]) $(id).addEventListener("click", () => fileInput.click());
  fileInput.addEventListener("change", async () => {
    await addFiles(fileInput.files);
    fileInput.value = "";
  });

  const dropzone = $("dropzone");
  for (const type of ["dragenter", "dragover"]) {
    dropzone.addEventListener(type, (event) => {
      event.preventDefault();
      dropzone.classList.add("dragging");
    });
  }
  for (const type of ["dragleave", "drop"]) {
    dropzone.addEventListener(type, (event) => {
      event.preventDefault();
      dropzone.classList.remove("dragging");
    });
  }
  dropzone.addEventListener("drop", (event) => addFiles(event.dataTransfer.files));
  // A file dropped outside the drop area must not make the page open it.
  for (const type of ["dragover", "drop"]) {
    window.addEventListener(type, (event) => event.preventDefault());
  }

  $("xml-text").addEventListener("input", showInput);
  $("convert").addEventListener("click", () => {
    convertAll();
    $("resultado").scrollIntoView({ block: "start" });
  });
  $("save-all").addEventListener("click", saveAll);

  for (const id of ["issuer-lines", "footer", "show-qr", "use-template", "template-target", "template"]) {
    $(id).addEventListener("input", scheduleRender);
  }
  $("logo").addEventListener("change", async () => {
    const [file] = $("logo").files;
    if (!file) return;
    state.logo = new Uint8Array(await file.arrayBuffer());
    $("logo-remove").hidden = false;
    scheduleRender();
  });
  $("logo-remove").addEventListener("click", () => {
    state.logo = null;
    $("logo").value = "";
    $("logo-remove").hidden = true;
    scheduleRender();
  });
  $("template-load").addEventListener("click", () => {
    const format = $("template-target").value === "ticket" ? "ticket80" : "a4";
    $("template").value = builtinTemplate(format);
    scheduleRender();
  });
}

async function main() {
  bindEvents();
  try {
    await init();
  } catch (error) {
    setStatus(`No se pudo cargar el motor de conversión: ${errorText(error)}`, true);
    return;
  }
  ready = true;
  loadOptions();
  fillOptions();
  showInput();
  setStatus("Listo. Pega, arrastra o elige tu XML.");
  try {
    await platform.start(currentOptions());
  } catch (error) {
    setStatus(errorText(error), true);
  }
}

main();
