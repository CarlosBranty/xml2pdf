// Format designer: a large preview of a document where each block of the
// design can be dragged to another place, plus a panel to edit the selected
// block and the style. A design is plain data (rows of blocks) that the
// engine renders; this file only edits that data.

const FAMILY = { a4: "page", a5: "page", letter: "page", ticket80: "ticket", ticket58: "ticket" };
const SETTING = { page: "page_design", ticket: "ticket_design" };
const DEFAULT_PRESET = "clasico";
const NEW_TEXT = "Escribe aquí tu texto";
const EXPORT_NAME = "diseno-xml2pdf.json";
const JSON_TYPE = { description: "Diseño", mime: "application/json", extension: ".json" };

const $ = (id) => document.getElementById(id);

/**
 * Sets up the designer.
 *
 * - `engine`: `{ designPreview, designCatalog, presetDesign }` from the WebAssembly module.
 * - `settings()`: the live settings object; designs are stored in it.
 * - `logo()`: logo bytes or `null`.
 * - `sample()`: bytes of the XML shown in the preview.
 * - `changed()`: called after the design changes, to save and re-render.
 * - `save(blob, name, type)`: saves a file through the platform.
 *
 * Returns `{ refresh }`, to draw the preview again when something else changed.
 */
export function createDesigner({ engine, settings, logo, sample, changed, save }) {
  const catalog = JSON.parse(engine.designCatalog());
  const label = (list, id) => list.find((item) => item.id === id)?.label ?? id;

  /** Block being edited: `{ row, block }` indexes into the design, or `null`. */
  let selected = null;
  /** What is being dragged: `{ from: { row, block } }` or `{ kind }` from the palette. */
  let dragging = null;
  let previewUrl = "";

  const family = () => FAMILY[$("design-format").value];
  const preset = (id) => JSON.parse(engine.presetDesign(id, family()));
  /** The design being edited; the classic preset until the user changes something. */
  const design = () => settings()[SETTING[family()]] ?? preset(DEFAULT_PRESET);
  const blockAt = (place, source = design()) => source.rows[place.row]?.blocks[place.block];

  function setDesign(next) {
    settings()[SETTING[family()]] = next;
    changed();
    refresh();
  }

  /** Applies a change to a copy of the design and stores it. */
  function edit(change) {
    const next = structuredClone(design());
    change(next);
    next.rows = next.rows.filter((row) => row.blocks.length > 0);
    setDesign(next);
  }

  function locate(source, target) {
    for (const [row, entry] of source.rows.entries()) {
      const block = entry.blocks.indexOf(target);
      if (block >= 0) return { row, block };
    }
    return null;
  }

  /**
   * Puts the dragged block next to `target`: `above` or `below` make a new
   * row, `left` or `right` share the row when it still has room.
   */
  function drop(target, zone) {
    const source = dragging;
    dragging = null;
    if (!source) return;
    if (source.from && source.from.row === target.row && source.from.block === target.block) return;

    edit((next) => {
      const targetRow = next.rows[target.row];
      const targetBlock = targetRow.blocks[target.block];
      const moving = source.from
        ? next.rows[source.from.row].blocks.splice(source.from.block, 1)[0]
        : newBlock(source.kind);

      const beside = zone === "left" || zone === "right";
      if (beside && targetRow.blocks.length < catalog.limits.blocks_per_row) {
        const at = targetRow.blocks.indexOf(targetBlock) + (zone === "right" ? 1 : 0);
        targetRow.blocks.splice(at, 0, moving);
      } else {
        const before = zone === "above" || zone === "left";
        next.rows.splice(next.rows.indexOf(targetRow) + (before ? 0 : 1), 0, { blocks: [moving] });
      }
      next.rows = next.rows.filter((row) => row.blocks.length > 0);
      selected = locate(next, moving);
    });
  }

  const newBlock = (kind) => (kind === "text" ? { type: kind, text: NEW_TEXT } : { type: kind });

  function addBlock(kind) {
    edit((next) => {
      const block = newBlock(kind);
      next.rows.push({ blocks: [block] });
      selected = locate(next, block);
    });
  }

  /** Part of a block the pointer is over: its top or bottom quarter, or a side. */
  function zoneOf(event, element) {
    const box = element.getBoundingClientRect();
    const y = (event.clientY - box.top) / box.height;
    if (y < 0.25) return "above";
    if (y > 0.75) return "below";
    return (event.clientX - box.left) / box.width < 0.5 ? "left" : "right";
  }

  // --- Drawing ---------------------------------------------------------------

  function handle(rect, size) {
    const [row, block] = rect.id.slice(1).split("b").map(Number);
    const place = { row, block };
    const element = document.createElement("div");
    element.className = "design-block";
    element.draggable = true;
    element.dataset.id = rect.id;
    element.title = "Arrastra para mover · clic para editar";
    element.classList.toggle("selected", selected?.row === row && selected?.block === block);
    element.style.left = `${(rect.x / size.width) * 100}%`;
    element.style.top = `${(rect.y / size.height) * 100}%`;
    element.style.width = `${(rect.width / size.width) * 100}%`;
    element.style.height = `${(Math.min(rect.height, size.height - rect.y) / size.height) * 100}%`;

    const name = document.createElement("span");
    name.textContent = label(catalog.blocks, blockAt(place)?.type);
    element.append(name);

    element.addEventListener("click", () => {
      selected = place;
      refresh();
    });
    element.addEventListener("dragstart", (event) => {
      dragging = { from: place };
      event.dataTransfer?.setData("text/plain", rect.id);
    });
    element.addEventListener("dragover", (event) => {
      if (!dragging) return;
      event.preventDefault();
      element.dataset.zone = zoneOf(event, element);
    });
    element.addEventListener("dragleave", () => delete element.dataset.zone);
    element.addEventListener("drop", (event) => {
      event.preventDefault();
      drop(place, zoneOf(event, element));
    });
    return element;
  }

  function drawPreview() {
    const format = $("design-format").value;
    const bytes = sample();
    if (!bytes) return;
    let preview;
    try {
      const json = JSON.stringify({ ...settings(), format });
      preview = engine.designPreview(bytes, json, logo() ?? undefined);
    } catch (error) {
      $("design-error").textContent = String(error?.message ?? error);
      return;
    }
    $("design-error").textContent = "";

    const size = { width: preview.widthPt, height: preview.heightPt };
    const rects = JSON.parse(preview.blocks);
    URL.revokeObjectURL(previewUrl);
    previewUrl = URL.createObjectURL(new Blob([preview.svg], { type: "image/svg+xml" }));
    preview.free();

    $("design-image").src = previewUrl;
    // Reserves the room of the preview at once, so the handles are in place
    // before the image has loaded.
    $("design-canvas").style.aspectRatio = `${size.width} / ${size.height}`;
    $("design-canvas").classList.toggle("ticket", family() === "ticket");
    $("design-overlays").replaceChildren(...rects.map((rect) => handle(rect, size)));
  }

  function drawStructure() {
    const current = design();
    $("design-structure").replaceChildren(
      ...current.rows.map((row, rowIndex) => {
        const item = document.createElement("li");
        for (const [blockIndex, block] of row.blocks.entries()) {
          const chip = document.createElement("button");
          chip.type = "button";
          chip.className = "pill";
          chip.textContent = label(catalog.blocks, block.type);
          const isSelected = selected?.row === rowIndex && selected?.block === blockIndex;
          chip.setAttribute("aria-pressed", String(isSelected));
          chip.addEventListener("click", () => {
            selected = { row: rowIndex, block: blockIndex };
            refresh();
          });
          item.append(chip);
        }
        return item;
      }),
    );
  }

  function drawStyle() {
    const { style = {} } = design();
    $("design-text").value = style.text ?? "normal";
    $("design-boxes").value = style.boxes ?? "rounded";
    $("design-heading").value = style.heading ?? "shaded";
    $("design-stripes").checked = style.stripes ?? true;
    $("design-filler").checked = style.filler_rows ?? true;
  }

  function drawBlockEditor() {
    const block = selected && blockAt(selected);
    $("design-block").hidden = !block;
    if (!block) {
      selected = null;
      return;
    }
    $("design-block-name").textContent = label(catalog.blocks, block.type);
    $("design-block-width").value = block.width ?? 1;
    $("design-block-boxed").value = block.boxed === undefined ? "" : String(block.boxed);
    $("design-block-align").value = block.align ?? "";
    $("design-block-text-field").hidden = block.type !== "text";
    $("design-block-text").value = block.text ?? "";
    $("design-block-columns-field").hidden = block.type !== "items";

    const shown = block.columns ?? ["quantity", "unit_code", "code", "description", "unit_value", "unit_price", "total"];
    $("design-block-columns").replaceChildren(
      ...catalog.columns.map((column) => {
        const option = document.createElement("label");
        option.className = "check";
        const box = document.createElement("input");
        box.type = "checkbox";
        box.value = column.id;
        box.checked = shown.includes(column.id);
        box.addEventListener("input", () => {
          const chosen = [...$("design-block-columns").querySelectorAll("input:checked")].map((input) => input.value);
          // The table needs a column; the last one cannot be unticked.
          if (chosen.length === 0) {
            box.checked = true;
            return;
          }
          editSelected((target) => (target.columns = chosen));
        });
        option.append(box, ` ${column.label}`);
        return option;
      }),
    );
  }

  function refresh() {
    drawPreview();
    drawStructure();
    drawStyle();
    drawBlockEditor();
  }

  // --- Controls --------------------------------------------------------------

  const editSelected = (change) => {
    if (!selected) return;
    const place = selected;
    edit((next) => change(blockAt(place, next)));
  };
  const editStyle = (change) => edit((next) => change((next.style ??= {})));

  /** Stores an optional value, removing the key when it goes back to the default. */
  const optional = (target, key, value) => {
    if (value === undefined) delete target[key];
    else target[key] = value;
  };

  function bind() {
    $("design-format").addEventListener("input", () => {
      selected = null;
      refresh();
    });

    $("design-presets").replaceChildren(
      ...catalog.presets.map((item) => {
        const button = document.createElement("button");
        button.type = "button";
        button.className = "pill";
        button.dataset.preset = item.id;
        button.textContent = item.label;
        button.addEventListener("click", () => {
          selected = null;
          setDesign(preset(item.id));
        });
        return button;
      }),
    );

    $("design-palette").replaceChildren(
      ...catalog.blocks.map((item) => {
        const chip = document.createElement("button");
        chip.type = "button";
        chip.className = "pill";
        chip.draggable = true;
        chip.dataset.kind = item.id;
        chip.textContent = `+ ${item.label}`;
        chip.title = "Clic para agregarlo al final, o arrástralo a la vista previa";
        chip.addEventListener("click", () => addBlock(item.id));
        chip.addEventListener("dragstart", (event) => {
          dragging = { kind: item.id };
          event.dataTransfer?.setData("text/plain", item.id);
        });
        return chip;
      }),
    );

    $("design-text").addEventListener("input", (event) => editStyle((style) => (style.text = event.target.value)));
    $("design-boxes").addEventListener("input", (event) => editStyle((style) => (style.boxes = event.target.value)));
    $("design-heading").addEventListener("input", (event) => editStyle((style) => (style.heading = event.target.value)));
    $("design-stripes").addEventListener("input", (event) => editStyle((style) => (style.stripes = event.target.checked)));
    $("design-filler").addEventListener("input", (event) => editStyle((style) => (style.filler_rows = event.target.checked)));

    $("design-block-width").addEventListener("input", (event) => {
      const width = Math.min(Math.max(Number.parseInt(event.target.value, 10) || 1, 1), catalog.limits.width);
      editSelected((block) => optional(block, "width", width === 1 ? undefined : width));
    });
    $("design-block-boxed").addEventListener("input", (event) => {
      const value = event.target.value;
      editSelected((block) => optional(block, "boxed", value === "" ? undefined : value === "true"));
    });
    $("design-block-align").addEventListener("input", (event) => {
      editSelected((block) => optional(block, "align", event.target.value || undefined));
    });
    $("design-block-text").addEventListener("change", (event) => {
      const text = event.target.value.slice(0, catalog.limits.text);
      editSelected((block) => (block.text = text));
    });
    $("design-block-remove").addEventListener("click", () => {
      if (!selected) return;
      const place = selected;
      selected = null;
      edit((next) => next.rows[place.row].blocks.splice(place.block, 1));
    });

    $("design-reset").addEventListener("click", () => {
      selected = null;
      setDesign(null);
    });
    $("design-export").addEventListener("click", async () => {
      const { page_design, ticket_design } = settings();
      const content = JSON.stringify({ page_design, ticket_design }, null, 2);
      const message = await save(new Blob([content], { type: JSON_TYPE.mime }), EXPORT_NAME, JSON_TYPE);
      if (message) $("design-error").textContent = "";
    });
    $("design-import").addEventListener("click", () => $("design-import-file").click());
    $("design-import-file").addEventListener("change", async (event) => {
      const [file] = event.target.files;
      event.target.value = "";
      if (!file) return;
      importDesigns(await file.text());
    });
  }

  /** Replaces the designs with the ones of an exported file, if the engine accepts them. */
  function importDesigns(text) {
    const previous = { page_design: settings().page_design, ticket_design: settings().ticket_design };
    try {
      const imported = JSON.parse(text);
      settings().page_design = imported.page_design ?? null;
      settings().ticket_design = imported.ticket_design ?? null;
      // Rendering both families checks the file before it is kept.
      for (const format of ["a4", "ticket80"]) {
        engine.designPreview(sample(), JSON.stringify({ ...settings(), format }), undefined).free();
      }
    } catch (error) {
      Object.assign(settings(), previous);
      $("design-error").textContent = `No se pudo importar el diseño: ${error?.message ?? error}`;
      return;
    }
    selected = null;
    changed();
    refresh();
  }

  bind();
  refresh();
  return { refresh };
}
