// xml2pdf desktop window. The work happens in the Rust side; this file shows
// the configuration and the log of processed documents.

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const SAVE_DELAY_MS = 400;
const MAX_LOG_ENTRIES = 200;

const $ = (id) => document.getElementById(id);
let config = null;
let watching = false;
let saveTimer = 0;

function showMessage(text, isError = false) {
  $("message").textContent = text;
  $("message").hidden = !text;
  $("message").classList.toggle("error", isError);
}

async function call(command, args) {
  try {
    const result = await invoke(command, args);
    showMessage("");
    return result;
  } catch (error) {
    showMessage(String(error), true);
    throw error;
  }
}

// --- Rendering -------------------------------------------------------------

function fileName(path) {
  return path.split(/[\\/]/).pop();
}

function showConfig() {
  $("watch-dir").textContent = config.watch_dir ?? "Sin elegir";
  $("output-dir").textContent =
    config.output_dir ?? (config.watch_dir ? `${config.watch_dir} › pdf` : "Subcarpeta «pdf»");
  $("watch-on-start").checked = config.watch_on_start;
  $("print").checked = config.print;
  $("printer").value = config.printer ?? "";
  $("copies").value = config.copies;
  $("format").value = config.settings.format;
  $("accent").value = config.settings.accent_color;
  $("issuer-lines").value = config.settings.issuer_lines.join("\n");
  $("footer").value = config.settings.footer ?? "";
  $("show-qr").checked = config.settings.show_qr;
  $("logo").textContent = config.logo ? fileName(config.logo) : "Sin logo";
  $("template").textContent = config.template ? fileName(config.template) : "Plantilla integrada";
}

function showWatching() {
  $("toggle-watch").textContent = watching ? "Detener vigilancia" : "Iniciar vigilancia";
  $("watch-state").textContent = watching
    ? `Vigilando ${config.watch_dir}`
    : "Vigilancia detenida";
  $("watch-state").classList.toggle("active", watching);
}

function showPrinters(printers, error) {
  const options = [new Option("Predeterminada del sistema", "")];
  const names = new Set(printers);
  if (config.printer) names.add(config.printer);
  for (const name of names) options.push(new Option(name, name));
  $("printer").replaceChildren(...options);
  $("printer-error").textContent = error ?? "";
  $("printer-error").hidden = !error;
}

function addLogEntry(event) {
  const item = document.createElement("li");
  const time = new Date().toLocaleTimeString("es-PE", { hour: "2-digit", minute: "2-digit" });

  if (event.kind === "failed") {
    item.className = "error";
    item.textContent = `${time} · ${fileName(event.source)}: ${event.message}`;
  } else {
    const status = event.print_error
      ? `PDF creado, no se imprimió: ${event.print_error}`
      : event.printed
        ? "PDF creado e impreso"
        : "PDF creado";
    item.classList.toggle("error", Boolean(event.print_error));
    item.textContent = `${time} · ${fileName(event.pdf)} — ${status}`;
    item.title = event.pdf;
  }

  $("log").prepend(item);
  while ($("log").children.length > MAX_LOG_ENTRIES) $("log").lastElementChild.remove();
  $("log-empty").hidden = true;
}

// --- Saving ----------------------------------------------------------------

function readForm() {
  config.watch_on_start = $("watch-on-start").checked;
  config.print = $("print").checked;
  config.printer = $("printer").value || null;
  config.copies = Number.parseInt($("copies").value, 10) || 1;
  config.settings.format = $("format").value;
  config.settings.accent_color = $("accent").value;
  config.settings.issuer_lines = $("issuer-lines")
    .value.split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  config.settings.footer = $("footer").value.trim() || null;
  config.settings.show_qr = $("show-qr").checked;
}

async function save() {
  await call("save_config", { config });
  showConfig();
  showWatching();
}

function scheduleSave() {
  readForm();
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => save().catch(() => {}), SAVE_DELAY_MS);
}

/** Runs a dialog command and stores its result under `key`. */
async function choose(command, key, args) {
  const path = await call(command, args);
  if (!path) return;
  config[key] = path;
  await save();
}

async function reset(key) {
  config[key] = null;
  await save();
}

// --- Events ----------------------------------------------------------------

function bindEvents() {
  const fields = ["watch-on-start", "print", "printer", "copies", "format", "accent", "issuer-lines", "footer", "show-qr"];
  for (const id of fields) $(id).addEventListener("input", scheduleSave);

  $("pick-watch-dir").addEventListener("click", () => choose("pick_folder", "watch_dir"));
  $("pick-output-dir").addEventListener("click", () => choose("pick_folder", "output_dir"));
  $("reset-output-dir").addEventListener("click", () => reset("output_dir"));
  $("pick-logo").addEventListener("click", () => choose("pick_file", "logo", { kind: "logo" }));
  $("reset-logo").addEventListener("click", () => reset("logo"));
  $("pick-template").addEventListener("click", () => choose("pick_file", "template", { kind: "template" }));
  $("reset-template").addEventListener("click", () => reset("template"));

  $("toggle-watch").addEventListener("click", async () => {
    await call(watching ? "stop_watching" : "start_watching");
    watching = !watching;
    showWatching();
  });

  $("convert").addEventListener("click", async () => {
    const events = await call("convert_files");
    events.forEach(addLogEntry);
  });
}

async function main() {
  const snapshot = await call("snapshot");
  config = snapshot.config;
  watching = snapshot.watching;
  showPrinters(snapshot.printers, snapshot.printer_error);
  showConfig();
  showWatching();
  bindEvents();
  await listen("watch-event", (event) => addLogEntry(event.payload));
}

main();
