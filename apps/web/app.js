// xml2pdf web app: converts SUNAT XML documents to PDF inside the browser.
// The conversion runs in WebAssembly; no file is sent to a server.

import init, { convert, builtinTemplate, defaultSettings } from "./pkg/xml2pdf_wasm.js";

const STORAGE_KEY = "xml2pdf.state.v2";
const RENDER_DELAY_MS = 350;
const PASTED_NAME = "pegado.xml";

/** Formats shown after converting, in display order. */
const FORMATS = [
  { id: "a4", title: "A4", size: "210 × 297 mm", family: "page" },
  { id: "a5", title: "A5", size: "148 × 210 mm", family: "page" },
  { id: "ticket80", title: "Ticket 80", size: "80 mm", family: "ticket" },
  { id: "ticket58", title: "Ticket 58", size: "58 mm", family: "ticket" },
];

const $ = (id) => document.getElementById(id);

const state = {
  /** Files chosen or dropped: `{ name, bytes }`. */
  files: [],
  /** Converted documents: `{ name, formats: { [formatId]: { fileName, blob, pdfUrl, previewUrl } } }`. */
  documents: [],
  /** Inputs that could not be converted: `{ name, message }`. */
  errors: [],
  selected: 0,
  settings: {},
  /** Logo image as a `Uint8Array`, or `null`. */
  logo: null,
  template: "",
  templateTarget: "page",
  useTemplate: false,
};

let ready = false;
let renderTimer = 0;

function setStatus(message, isError = false) {
  $("status").textContent = message;
  $("status").classList.toggle("error", isError);
}

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

// --- Input -----------------------------------------------------------------

/** Everything to convert: the chosen files plus the pasted text, if any. */
function inputs() {
  const pasted = $("xml-text").value.trim();
  const text = pasted ? [{ name: PASTED_NAME, bytes: new TextEncoder().encode(pasted) }] : [];
  return [...state.files, ...text];
}

function showInput() {
  $("files").replaceChildren(
    ...state.files.map((file, index) => {
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
    }),
  );
  $("convert").disabled = !ready || inputs().length === 0;
}

async function addFiles(fileList) {
  for (const file of Array.from(fileList)) {
    state.files.push({ name: file.name, bytes: new Uint8Array(await file.arrayBuffer()) });
  }
  showInput();
  if (state.files.length > 0) setStatus("Archivo listo. Presiona Convertir.");
}

// --- Conversion ------------------------------------------------------------

function releaseDocuments() {
  for (const document of state.documents) {
    for (const file of Object.values(document.formats)) {
      URL.revokeObjectURL(file.pdfUrl);
      URL.revokeObjectURL(file.previewUrl);
    }
  }
  state.documents = [];
  state.errors = [];
}

/** Converts one input into every format. Returns one entry per document it contains. */
function convertInput(input) {
  const documents = [];
  for (const format of FORMATS) {
    const settings = JSON.stringify({ ...state.settings, format: format.id });
    const template =
      state.useTemplate && state.templateTarget === format.family ? state.template : undefined;
    const files = convert(input.name, input.bytes, settings, state.logo ?? undefined, template);

    files.forEach((file, index) => {
      documents[index] ??= { name: file.name.replace(/\.pdf$/, ""), formats: {} };
      const blob = new Blob([file.pdf], { type: "application/pdf" });
      const preview = new Blob([file.preview], { type: "image/svg+xml" });
      documents[index].formats[format.id] = {
        fileName: `${documents[index].name}-${format.id}.pdf`,
        blob,
        pdfUrl: URL.createObjectURL(blob),
        previewUrl: URL.createObjectURL(preview),
      };
      file.free();
    });
  }
  return documents;
}

function convertAll() {
  const pending = inputs();
  if (pending.length === 0) {
    setStatus("Pega, arrastra o elige un XML primero.", true);
    return;
  }

  releaseDocuments();
  for (const input of pending) {
    try {
      state.documents.push(...convertInput(input));
    } catch (error) {
      state.errors.push({ name: input.name, message: String(error.message ?? error) });
    }
  }
  state.selected = Math.min(state.selected, Math.max(state.documents.length - 1, 0));
  showResults();
}

/** Converts again after an option changes, when there is something on screen. */
function scheduleRender() {
  readOptions();
  saveOptions();
  if ($("resultado").hidden) return;
  clearTimeout(renderTimer);
  renderTimer = setTimeout(convertAll, RENDER_DELAY_MS);
}

// --- Results ---------------------------------------------------------------

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
  save.addEventListener("click", () => savePdf(file));
  const print = document.createElement("button");
  print.type = "button";
  print.className = "button";
  print.textContent = "Imprimir";
  print.addEventListener("click", () => printPdf(file));
  actions.append(save, print);

  card.append(header, sheet, actions);
  return card;
}

function showResults() {
  const current = state.documents[state.selected];

  $("documents").replaceChildren(
    ...(state.documents.length > 1 ? state.documents : []).map((item, index) => {
      const tab = document.createElement("button");
      tab.type = "button";
      tab.className = "pill";
      tab.setAttribute("role", "tab");
      tab.textContent = item.name;
      tab.setAttribute("aria-selected", String(index === state.selected));
      tab.addEventListener("click", () => {
        state.selected = index;
        showResults();
      });
      return tab;
    }),
  );

  $("errors").replaceChildren(
    ...state.errors.map((error) => {
      const item = document.createElement("li");
      // Conversion errors already start with the name of the failing document.
      const named = error.message.startsWith(`${error.name}:`);
      item.textContent = named ? error.message : `${error.name}: ${error.message}`;
      return item;
    }),
  );

  $("formats").replaceChildren(
    ...(current ? FORMATS.map((format) => formatCard(format, current.formats[format.id])) : []),
  );
  $("save-hint").textContent = window.showSaveFilePicker
    ? "Guardar te deja elegir la carpeta y el nombre del archivo."
    : "En este navegador, Guardar descarga el PDF en tu carpeta de descargas. Chrome y Edge permiten elegir la carpeta.";
  $("save-hint").hidden = !current;
  $("resultado").hidden = false;

  const failed = state.errors.length;
  if (state.documents.length === 0) {
    setStatus("No se pudo convertir. Revisa el detalle más abajo.", true);
  } else if (failed > 0) {
    setStatus(`${state.documents.length} comprobante(s) convertido(s), ${failed} con error.`, true);
  } else {
    setStatus(
      state.documents.length === 1
        ? "Listo: tu PDF está en los cuatro formatos."
        : `Listo: ${state.documents.length} comprobantes en los cuatro formatos.`,
    );
  }
}

/** Saves a PDF, letting the user choose the location where the browser supports it. */
async function savePdf(file) {
  if (window.showSaveFilePicker) {
    try {
      const handle = await window.showSaveFilePicker({
        suggestedName: file.fileName,
        types: [{ description: "PDF", accept: { "application/pdf": [".pdf"] } }],
      });
      const writable = await handle.createWritable();
      await writable.write(file.blob);
      await writable.close();
      setStatus(`Guardado: ${handle.name}`);
      return;
    } catch (error) {
      if (error.name === "AbortError") return;
      // Any other failure falls back to a regular download.
    }
  }
  const link = document.createElement("a");
  link.href = file.pdfUrl;
  link.download = file.fileName;
  link.click();
  setStatus(`Descargado: ${file.fileName}`);
}

/** Opens the print dialog for a PDF through a hidden frame. */
function printPdf(file) {
  document.querySelector(".print-frame")?.remove();
  const frame = document.createElement("iframe");
  frame.className = "print-frame";
  frame.title = "Impresión";
  frame.addEventListener("load", () => {
    try {
      frame.contentWindow.focus();
      frame.contentWindow.print();
    } catch {
      // Some browsers do not let a page print an embedded PDF: open it instead.
      window.open(file.pdfUrl, "_blank", "noopener");
      setStatus("Se abrió el PDF en otra pestaña; imprímelo desde allí.");
    }
  });
  frame.src = file.pdfUrl;
  document.body.append(frame);
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

  $("xml-text").addEventListener("input", showInput);
  $("convert").addEventListener("click", () => {
    convertAll();
    $("resultado").scrollIntoView({ block: "start" });
  });

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
    setStatus(`No se pudo cargar el motor de conversión: ${error.message ?? error}`, true);
    return;
  }
  ready = true;
  loadOptions();
  fillOptions();
  showInput();
  setStatus("Listo. Pega, arrastra o elige tu XML.");
}

main();
