// Platform integration for the browser: saving and printing go through the
// browser itself. The desktop app replaces this file with its own version
// (apps/desktop/ui/platform.js), which talks to the operating system.

/** Explains to the user what Guardar does on this platform. */
export function saveHint() {
  return window.showSaveFilePicker
    ? "Guardar te deja elegir la carpeta y el nombre del archivo."
    : "En este navegador, Guardar descarga el archivo en tu carpeta de descargas. Chrome y Edge permiten elegir la carpeta.";
}

/**
 * Saves a file, letting the user choose the location where the browser
 * supports it. Resolves to a message for the user, or `null` when cancelled.
 */
export async function save(blob, fileName, type) {
  if (window.showSaveFilePicker) {
    try {
      const handle = await window.showSaveFilePicker({
        suggestedName: fileName,
        types: [{ description: type.description, accept: { [type.mime]: [type.extension] } }],
      });
      const writable = await handle.createWritable();
      await writable.write(blob);
      await writable.close();
      return `Guardado: ${handle.name}`;
    } catch (error) {
      if (error.name === "AbortError") return null;
      // Any other failure falls back to a regular download.
    }
  }
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = fileName;
  link.click();
  // The download has started by the next task; the URL is no longer needed.
  setTimeout(() => URL.revokeObjectURL(url), 60_000);
  return `Descargado: ${fileName}`;
}

/**
 * Opens the print dialog for a rendered PDF (`{ pdfUrl, ... }`) through a
 * hidden frame. Resolves to a message for the user, or `null`.
 */
export function print(file) {
  return new Promise((resolve) => {
    document.querySelector(".print-frame")?.remove();
    const frame = document.createElement("iframe");
    frame.className = "print-frame";
    frame.title = "Impresión";
    frame.addEventListener("load", () => {
      try {
        frame.contentWindow.focus();
        frame.contentWindow.print();
        resolve(null);
      } catch {
        // Some browsers do not let a page print an embedded PDF: open it instead.
        window.open(file.pdfUrl, "_blank", "noopener");
        resolve("Se abrió el PDF en otra pestaña; imprímelo desde allí.");
      }
    });
    frame.src = file.pdfUrl;
    document.body.append(frame);
  });
}

/** Called once the app is ready, with the current options. Nothing to set up in a browser. */
export async function start(_options) {}

/** Called whenever the user changes an option. The browser keeps them in the page only. */
export function optionsChanged(_options) {}
