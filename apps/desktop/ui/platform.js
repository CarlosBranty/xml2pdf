// Platform integration for the desktop app. It replaces apps/web/platform.js
// in the desktop build: saving uses the system dialog, printing goes straight
// to the configured printer, and a panel controls the folder watcher.

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const APPLY_DELAY_MS = 400;
const MAX_LOG_ENTRIES = 200;
/** Template family of each format, to know when a custom template applies. */
const FAMILY = { a4: "page", a5: "page", ticket80: "ticket", ticket58: "ticket" };

const $ = (id) => document.getElementById(id);

/** Watcher configuration, as the backend stores it. */
let config = null;
/** Document options of the shared interface. */
let options = null;
let watching = false;
let applyTimer = 0;

const toBytes = async (blob) => Array.from(new Uint8Array(await blob.arrayBuffer()));
const fileName = (path) => path.split(/[\\/]/).pop();

/** Rewrites backend errors the user is likely to meet into plain Spanish. */
function friendly(error) {
  const message = String(error?.message ?? error);
  if (message.includes("Install CUPS")) {
    return "No se encontró el sistema de impresión (CUPS). Instálalo y configura una impresora para poder imprimir.";
  }
  if (message.includes("Install SumatraPDF")) {
    return "No se pudo imprimir. Instala SumatraPDF para imprimir sin ventanas en Windows.";
  }
  if (message.includes("choose a folder to watch")) return "Elige primero la carpeta a vigilar.";
  return message;
}

export function saveHint() {
  return "Guardar abre el diálogo del sistema para elegir la carpeta. Imprimir envía el PDF a la impresora elegida en «Impresión automática».";
}

export async function save(blob, name, type) {
  try {
    const path = await invoke("save_file", {
      name,
      bytes: await toBytes(blob),
      description: type.description,
      extension: type.extension.slice(1),
    });
    return path ? `Guardado: ${path}` : null;
  } catch (error) {
    throw new Error(friendly(error));
  }
}

export async function print(file) {
  try {
    const printer = await invoke("print_pdf", {
      name: file.fileName,
      bytes: await toBytes(file.blob),
      format: file.formatId,
      pageMm: file.pageMm,
    });
    return `Enviado a la impresora: ${printer}`;
  } catch (error) {
    throw new Error(friendly(error));
  }
}

// --- Folder watcher panel --------------------------------------------------

function setMessage(text, isError = false) {
  $("watch-message").textContent = text;
  $("watch-message").classList.toggle("error", isError);
}

function showPanel() {
  $("watch-dir").textContent = config.watch_dir ?? "Sin elegir";
  $("output-dir").textContent =
    config.output_dir ?? (config.watch_dir ? `${config.watch_dir} › pdf` : "Subcarpeta «pdf»");
  $("watch-format").value = config.settings.format;
  $("printer").value = config.printer ?? "";
  $("copies").value = config.copies;
  $("auto-print").checked = config.print;
  $("watch-on-start").checked = config.watch_on_start;

  $("toggle-watch").textContent = watching ? "Detener vigilancia" : "Iniciar vigilancia";
  $("watch-state").textContent = watching ? "Vigilando" : "Detenida";
  $("watch-state").classList.toggle("active", watching);
}

function showPrinters(printers, error) {
  const names = new Set(printers);
  if (config.printer) names.add(config.printer);
  $("printer").replaceChildren(
    new Option("Predeterminada del sistema", ""),
    ...Array.from(names, (name) => new Option(name, name)),
  );
  $("printer-error").textContent = error ? friendly(error) : "";
  $("printer-error").hidden = !error;
}

function addLogEntry(event) {
  const item = document.createElement("li");
  const time = new Date().toLocaleTimeString("es-PE", { hour: "2-digit", minute: "2-digit" });

  if (event.kind === "failed") {
    item.className = "failed";
    item.textContent = `${time} · ${fileName(event.source)}: ${event.message}`;
  } else {
    const status = event.print_error
      ? `PDF creado, no se imprimió: ${friendly(event.print_error)}`
      : event.printed
        ? "PDF creado e impreso"
        : "PDF creado";
    item.classList.toggle("failed", Boolean(event.print_error));
    item.textContent = `${time} · ${fileName(event.pdf)} — ${status}`;
    item.title = event.pdf;
  }

  $("watch-log").prepend(item);
  while ($("watch-log").children.length > MAX_LOG_ENTRIES) $("watch-log").lastElementChild.remove();
  $("watch-log-empty").hidden = true;
}

function readPanel() {
  config.settings.format = $("watch-format").value;
  config.printer = $("printer").value || null;
  config.copies = Number.parseInt($("copies").value, 10) || 1;
  config.print = $("auto-print").checked;
  config.watch_on_start = $("watch-on-start").checked;
}

/** Sends the panel choices and the document options to the backend. */
async function apply() {
  const usesTemplate =
    options.useTemplate &&
    options.template.trim() !== "" &&
    options.templateTarget === FAMILY[config.settings.format];
  try {
    await invoke("apply_settings", {
      watch: {
        watch_dir: config.watch_dir,
        output_dir: config.output_dir,
        print: config.print,
        printer: config.printer,
        copies: config.copies,
        watch_on_start: config.watch_on_start,
        format: config.settings.format,
      },
      render: {
        issuer_lines: options.settings.issuer_lines,
        footer: options.settings.footer,
        show_qr: options.settings.show_qr,
        logo: options.logo ? Array.from(options.logo) : null,
        template: usesTemplate ? options.template : null,
      },
    });
    setMessage("");
  } catch (error) {
    setMessage(friendly(error), true);
  }
  showPanel();
}

function scheduleApply() {
  clearTimeout(applyTimer);
  applyTimer = setTimeout(apply, APPLY_DELAY_MS);
}

async function chooseFolder(key) {
  const path = await invoke("pick_folder");
  if (!path) return;
  config[key] = path;
  await apply();
}

function bindPanel() {
  for (const id of ["watch-format", "printer", "copies", "auto-print", "watch-on-start"]) {
    $(id).addEventListener("input", () => {
      readPanel();
      scheduleApply();
    });
  }
  $("pick-watch-dir").addEventListener("click", () => chooseFolder("watch_dir"));
  $("pick-output-dir").addEventListener("click", () => chooseFolder("output_dir"));
  $("reset-output-dir").addEventListener("click", async () => {
    config.output_dir = null;
    await apply();
  });
  $("toggle-watch").addEventListener("click", async () => {
    try {
      // Pending choices are saved first, so the watcher starts with them.
      clearTimeout(applyTimer);
      await apply();
      await invoke(watching ? "stop_watching" : "start_watching");
      watching = !watching;
      setMessage("");
    } catch (error) {
      setMessage(friendly(error), true);
    }
    showPanel();
  });
}

/** Adds the watcher panel to the page and loads its state from the backend. */
export async function start(current) {
  options = current;

  const styles = document.createElement("link");
  styles.rel = "stylesheet";
  styles.href = "desktop.css";
  document.head.append(styles);
  // The panel is a file of this app, not user content.
  const panel = await (await fetch("panel.html")).text();
  // Right below the converter, since it is the main reason to use the desktop app.
  $("resultado").insertAdjacentHTML("afterend", panel);
  const link = document.createElement("a");
  link.href = "#vigilancia";
  link.textContent = "Impresión automática";
  document.querySelector(".nav nav").append(link);

  const snapshot = await invoke("snapshot");
  config = snapshot.config;
  watching = snapshot.watching;
  showPrinters(snapshot.printers, snapshot.printer_error);
  showPanel();
  bindPanel();
  await listen("watch-event", (event) => addLogEntry(event.payload));
  // Keeps the stored options in step with the ones shown in this window.
  await apply();
}

export function optionsChanged(current) {
  options = current;
  if (config) scheduleApply();
}
