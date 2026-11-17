// Browser side of scripts/ui-check.py. It is injected into a copy of the
// built site, drives the page the way a user would and posts what it saw.
// It is never part of the published app.

const scenario = new URLSearchParams(location.search).get("scenario");
const $ = (id) => document.getElementById(id);
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function until(check, label, timeout = 60000) {
  const start = performance.now();
  while (!check()) {
    if (performance.now() - start > timeout) throw new Error(`timeout waiting for: ${label}`);
    await sleep(50);
  }
}

// The site policy forbids fetching blob URLs, so remember each blob by its URL.
const blobs = new Map();
const createObjectURL = URL.createObjectURL.bind(URL);
URL.createObjectURL = (blob) => {
  const url = createObjectURL(blob);
  blobs.set(url, blob);
  return url;
};

// Record downloads instead of saving files, and force the download path.
const downloads = [];
HTMLAnchorElement.prototype.click = function click() {
  downloads.push({ name: this.download, blob: blobs.get(this.href) });
};
delete window.showSaveFilePicker;

const fetchBytes = async (name) => new Uint8Array(await (await fetch(name)).arrayBuffer());
const rows = () => [...$("documents").children];
const selectedRow = () => rows().findIndex((row) => row.querySelector('[aria-current="true"]'));
const previewsDrawn = () => {
  const images = [...document.querySelectorAll(".sheet img")];
  return images.length > 0 && images.every((image) => image.complete && image.naturalWidth > 0);
};

async function ready() {
  await until(() => $("status").textContent.startsWith("Listo"), "the engine to load");
}

function chooseFiles(files) {
  const transfer = new DataTransfer();
  for (const file of files) transfer.items.add(file);
  $("file-input").files = transfer.files;
  $("file-input").dispatchEvent(new Event("change"));
}

async function paste(text) {
  $("xml-text").value = text;
  $("xml-text").dispatchEvent(new Event("input"));
  await until(() => !$("convert").disabled, "Convertir to be enabled");
  $("convert").click();
  await until(() => !$("resultado").hidden, "the result");
}

const scenarios = {
  /** Paste one XML, convert, and save one format. */
  async paste() {
    await ready();
    const disabledWhenEmpty = $("convert").disabled;
    await paste(new TextDecoder().decode(await fetchBytes("factura.xml")));
    await until(previewsDrawn, "the previews");
    document.querySelector(".format .save").click();
    await sleep(50);
    const structured = JSON.parse(document.querySelector('script[type="application/ld+json"]').textContent);
    return {
      disabledWhenEmpty,
      // The input sits to the left of the headline on a wide window.
      inputLeftOfHeadline:
        $("dropzone").getBoundingClientRect().right <=
        document.querySelector(".hero-copy").getBoundingClientRect().left + 1,
      seo: {
        title: document.title,
        language: document.documentElement.lang,
        description: document.querySelector('meta[name="description"]')?.content.length > 80,
        canonical: document.querySelector('link[rel="canonical"]')?.getAttribute("href") ?? null,
        structured: structured["@graph"].map((item) => item["@type"]),
        questions: document.querySelectorAll(".faq dt").length,
        answersInStructuredData: structured["@graph"][1].mainEntity.length,
      },
      status: $("status").textContent,
      title: $("results-title").textContent,
      formats: [...document.querySelectorAll(".format h3")].map((title) => title.textContent),
      batchHidden: $("batch").hidden,
      errors: $("errors").children.length,
      saved: downloads[0]?.name,
      savedHeader: await downloads[0]?.blob.slice(0, 5).text(),
    };
  },

  /** Paste something that is not a supported document. */
  async invalid() {
    await ready();
    await paste("<ApplicationResponse/>");
    return {
      status: $("status").textContent,
      errors: [...$("errors").children].map((item) => item.textContent),
      formats: document.querySelectorAll(".format").length,
    };
  },

  /** Choose 105 files, one of them broken: the batch limit and bulk saving. */
  async bulk() {
    await ready();
    const samples = await Promise.all(
      ["factura.xml", "boleta.xml", "nota-credito.xml", "nota-debito.xml"].map(fetchBytes),
    );
    const broken = new TextEncoder().encode("<ApplicationResponse/>");
    chooseFiles(
      Array.from({ length: 105 }, (_, index) => {
        const name = `lote-${String(index + 1).padStart(3, "0")}.xml`;
        return new File([index === 2 ? broken : samples[index % 4]], name, { type: "text/xml" });
      }),
    );
    await until(() => !$("convert").disabled, "the files to be added");
    const report = { addStatus: $("status").textContent };

    $("convert").click();
    await until(() => rows().length > 0, "the list of documents");
    await until(previewsDrawn, "the previews");
    Object.assign(report, {
      title: $("results-title").textContent,
      rows: rows().length,
      failedRows: rows().filter((row) => row.querySelector(".failed")).map((row) => row.textContent),
      selectedRow: selectedRow(),
      formats: document.querySelectorAll(".format").length,
      noticeHidden: $("notice").hidden,
    });

    const firstPdf = document.querySelector(".sheet").href;
    rows()[1].querySelector("button").click();
    await until(() => document.querySelector(".sheet").href !== firstPdf, "the new selection");
    report.selectedAfterClick = selectedRow();

    $("bulk-format").value = "ticket80";
    $("save-all").click();
    report.saveAllDisabledWhileRunning = $("save-all").disabled;
    await until(() => downloads.length > 0, "the ZIP file", 180000);
    report.bulkStatus = $("bulk-status").textContent;
    report.zipName = downloads[0].name;
    await fetch("/upload/batch.zip", { method: "POST", body: downloads[0].blob });
    return report;
  },

  /** The desktop interface, with a stand-in for the Tauri bridge. */
  async desktop() {
    await ready();
    await until(() => $("toggle-watch"), "the watcher panel");
    const stub = window.__tauriStub;
    const commands = () => stub.calls.map((call) => call.command);
    const last = (command) => stub.calls.findLast((call) => call.command === command)?.args;
    const header = (bytes) => String.fromCharCode(...bytes.slice(0, 5));
    await until(() => commands().includes("apply_settings"), "the first settings sync");
    const report = {
      startupCommands: commands(),
      printers: [...$("printer").options].map((option) => option.textContent),
      watchDir: $("watch-dir").textContent,
      outputDir: $("output-dir").textContent,
    };

    await paste(new TextDecoder().decode(await fetchBytes("factura.xml")));
    await until(previewsDrawn, "the previews");
    report.saveHint = $("save-hint").textContent;
    const cards = [...document.querySelectorAll(".format")];

    cards[0].querySelector(".save").click();
    await until(() => $("status").textContent.startsWith("Guardado"), "the save");
    const saved = last("save_file");
    report.save = {
      status: $("status").textContent,
      name: saved.name,
      extension: saved.extension,
      header: header(saved.bytes),
    };

    const ticket = cards.find((card) => card.querySelector("h3").textContent === "Ticket 80");
    ticket.querySelectorAll("button")[1].click();
    await until(() => $("status").textContent.startsWith("Enviado"), "the print");
    const printed = last("print_pdf");
    report.print = {
      status: $("status").textContent,
      name: printed.name,
      format: printed.format,
      width: Math.round(printed.pageMm[0]),
      taller: printed.pageMm[1] > 60,
      header: header(printed.bytes),
    };

    $("copies").value = "3";
    $("copies").dispatchEvent(new Event("input", { bubbles: true }));
    await until(() => last("apply_settings")?.watch.copies === 3, "the settings to be applied");
    report.applied = last("apply_settings");

    $("toggle-watch").click();
    await until(() => $("toggle-watch").textContent === "Detener vigilancia", "the watcher to start");
    report.watchState = $("watch-state").textContent;
    report.startedWatching = commands().includes("start_watching");

    stub.emit("watch-event", {
      kind: "processed",
      source: "/descargas/F001-1.xml",
      pdf: "/descargas/pdf/20123456789-01-F001-1.pdf",
      printed: true,
      print_error: null,
    });
    stub.emit("watch-event", { kind: "failed", source: "/descargas/roto.xml", message: "not valid XML" });
    // Entries start with the time of day, which is dropped here.
    report.log = [...$("watch-log").children].map((item) => item.textContent.split(" · ").slice(1).join(" · "));
    return report;
  },

  /** The format designer: drag blocks, presets, block options, formats, import. */
  async designer() {
    await ready();
    const handles = () => [...$("design-overlays").children];
    await until(() => handles().length > 0, "the designer preview");
    const canvas = () => $("design-canvas").getBoundingClientRect();
    // The page reserves the preview's room before its image arrives.
    const roomReserved = canvas().height > 100;
    const saved = () => JSON.parse(localStorage.getItem("xml2pdf.state.v2") ?? "{}").settings ?? {};
    /** Whether an element is really displayed, whatever its styles say. */
    const shown = (id) => getComputedStyle($(id)).display !== "none";
    const kinds = (design) => design?.rows.map((row) => row.blocks.map((block) => block.type)) ?? null;
    const input = (id, value, type = "input") => {
      $(id).value = value;
      $(id).dispatchEvent(new Event(type, { bubbles: true }));
    };
    /** Drags one block handle onto a side of another, as the pointer would. */
    const drag = (sourceId, targetId, side) => {
      const source = document.querySelector(`.design-block[data-id="${sourceId}"]`);
      const target = document.querySelector(`.design-block[data-id="${targetId}"]`);
      if (!source || !target) {
        const present = handles().map((handle) => handle.dataset.id).join(" ");
        throw new Error(`cannot drag ${sourceId} to ${targetId}; handles: ${present}`);
      }
      const box = target.getBoundingClientRect();
      const [x, y] = { left: [0.1, 0.5], right: [0.9, 0.5], above: [0.5, 0.1], below: [0.5, 0.9] }[side];
      const at = { clientX: box.left + box.width * x, clientY: box.top + box.height * y, bubbles: true, cancelable: true };
      // Mouse events carry the pointer position reliably when built by a script.
      source.dispatchEvent(new MouseEvent("dragstart", { bubbles: true }));
      target.dispatchEvent(new MouseEvent("dragover", at));
      const zone = target.dataset.zone;
      target.dispatchEvent(new MouseEvent("drop", at));
      return zone;
    };

    const report = {
      initialHandles: handles().map((handle) => handle.dataset.id),
      initialLabels: handles().slice(0, 2).map((handle) => handle.textContent),
      designBeforeEditing: saved().page_design ?? null,
      blockEditorHidden: !shown("design-block"),
      roomReserved,
    };
    window.uiCheckPartial = report;

    // The QR goes beside the document box; then the totals get a row above the customer.
    report.zoneBeside = drag("r7b1", "r0b1", "left");
    report.afterBeside = kinds(saved().page_design);
    report.zoneAbove = drag("r4b0", "r1b0", "above");
    report.afterAbove = kinds(saved().page_design);
    // A row that is already full gets the block on a row of its own instead.
    drag("r8b0", "r0b2", "right");
    report.afterFullRow = kinds(saved().page_design).slice(0, 2);
    report.selectedAfterDrag = document.querySelector(".design-block.selected")?.dataset.id ?? null;

    document.querySelector('[data-preset="moderno"]').click();
    report.preset = { style: saved().page_design.style, firstRow: kinds(saved().page_design)[0] };

    document.querySelector('.design-block[data-id="r0b1"]').click();
    report.blockEditor = {
      hidden: !shown("design-block"),
      name: $("design-block-name").textContent,
      // Only the options that apply to the chosen block are offered.
      textField: shown("design-block-text-field"),
      columnsField: shown("design-block-columns-field"),
    };
    input("design-block-width", "4");
    input("design-block-boxed", "false");
    input("design-block-align", "right");
    report.editedBlock = saved().page_design.rows[0].blocks[1];

    document.querySelector('#design-palette [data-kind="text"]').click();
    input("design-block-text", "Gracias por su compra", "change");
    report.addedText = saved().page_design.rows.at(-1).blocks;

    const itemsRow = saved().page_design.rows.findIndex((row) => row.blocks[0].type === "items");
    document.querySelector(`.design-block[data-id="r${itemsRow}b0"]`).click();
    const code = document.querySelector('#design-block-columns input[value="code"]');
    code.checked = false;
    code.dispatchEvent(new Event("input", { bubbles: true }));
    report.columns = saved().page_design.rows[itemsRow].blocks[0].columns;

    input("design-boxes", "square");
    report.boxes = saved().page_design.style.boxes;
    report.previewError = $("design-error").textContent;

    // An invalid file is refused and the design stays as it was.
    const before = JSON.stringify(saved().page_design);
    const transfer = new DataTransfer();
    transfer.items.add(new File(['{"page_design":{"rows":[{"blocks":[]}]}}'], "malo.json", { type: "application/json" }));
    $("design-import-file").files = transfer.files;
    $("design-import-file").dispatchEvent(new Event("change"));
    await until(() => $("design-error").textContent !== "", "the import error");
    report.importError = $("design-error").textContent;
    report.designKeptAfterBadImport = JSON.stringify(saved().page_design) === before;

    // Tickets have their own design.
    input("design-format", "ticket80");
    report.ticket = {
      canvasIsTicket: $("design-canvas").classList.contains("ticket"),
      handles: handles().length,
      designBeforeEditing: saved().ticket_design ?? null,
    };
    document.querySelector('#design-palette [data-kind="spacer"]').click();
    report.ticket.lastRow = kinds(saved().ticket_design).at(-1);
    report.pageDesignUntouched = JSON.stringify(saved().page_design) !== "null";

    // A document converts with the designs in place.
    await paste(new TextDecoder().decode(await fetchBytes("factura.xml")));
    await until(previewsDrawn, "the previews");
    report.converted = { formats: document.querySelectorAll(".format").length, errors: $("errors").children.length };

    input("design-format", "a4");
    $("design-reset").click();
    report.afterReset = {
      design: saved().page_design ?? null,
      handles: handles().length,
      blockEditorHidden: !shown("design-block"),
    };
    return report;
  },

  /** Choose one ZIP holding more documents than a batch accepts. */
  async ziplimit() {
    await ready();
    chooseFiles([new File([await fetchBytes("many.zip")], "many.zip", { type: "application/zip" })]);
    await until(() => !$("convert").disabled, "the file to be added");
    $("convert").click();
    await until(() => rows().length > 0, "the list of documents");
    return {
      rows: rows().length,
      title: $("results-title").textContent,
      noticeHidden: $("notice").hidden,
      notice: $("notice").textContent,
    };
  },
};

if (scenarios[scenario]) {
  let report;
  try {
    report = await scenarios[scenario]();
  } catch (error) {
    // What a scenario gathered before failing helps to see where it stopped.
    report = { error: String(error), status: $("status")?.textContent, partial: window.uiCheckPartial };
  }
  window.scrollTo(0, 0);
  await fetch(`/report/${scenario}`, { method: "POST", body: JSON.stringify(report) });
}
