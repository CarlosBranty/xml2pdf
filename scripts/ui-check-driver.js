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
    return {
      disabledWhenEmpty,
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
    report = { error: String(error), status: $("status")?.textContent };
  }
  window.scrollTo(0, 0);
  await fetch(`/report/${scenario}`, { method: "POST", body: JSON.stringify(report) });
}
