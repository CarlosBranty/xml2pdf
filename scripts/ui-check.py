#!/usr/bin/env python3
"""End-to-end check of the web app in a real browser.

Serves a copy of the built site with the Content-Security-Policy of
deploy/Caddyfile, opens it in headless Firefox once per scenario and verifies
what the page reported: pasting an XML, an invalid document, a 105-file batch
with bulk saving, and a ZIP over the batch limit. When the desktop interface
is built (dist/desktop), it is checked too, with a stand-in for the Tauri
bridge that records the commands the window sends.

Usage: python3 scripts/ui-check.py [dist/web] [--desktop DIR] [--screenshots DIR]
Requires Firefox. Build first with scripts/build-web.sh (and
scripts/build-desktop-ui.sh for the desktop interface).
"""

import argparse
import io
import json
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import zipfile
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCENARIOS = ["paste", "invalid", "bulk", "ziplimit", "designer"]
SCENARIO_TIMEOUT_SECONDS = 240
# A 1x1 transparent PNG, served once a scenario has reported.
PIXEL = bytes.fromhex(
    "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c489"
    "0000000d49444154789c6360000002000001e221bc330000000049454e44ae426082"
)


def desktop_security_policy() -> str:
    config = json.loads((ROOT / "apps/desktop/src-tauri/tauri.conf.json").read_text(encoding="utf-8"))
    return config["app"]["security"]["csp"]


def content_security_policy() -> str:
    caddyfile = (ROOT / "deploy/Caddyfile").read_text(encoding="utf-8")
    match = re.search(r'Content-Security-Policy "([^"]+)"', caddyfile)
    if not match:
        sys.exit("deploy/Caddyfile has no Content-Security-Policy")
    return match.group(1)


class Handler(SimpleHTTPRequestHandler):
    """Static files plus the endpoints the driver script talks to."""

    policy = ""
    desktop_policy = ""
    reports: dict = {}
    uploads: dict = {}
    reported: dict = {}

    def do_GET(self):
        if self.path.startswith("/wait/"):
            # Keeps the page loading until its scenario reports, so the
            # browser takes the screenshot, and exits, only then.
            self.reported[self.path.removeprefix("/wait/")].wait(SCENARIO_TIMEOUT_SECONDS)
            self.send_response(200)
            self.send_header("Content-Type", "image/png")
            self.send_header("Content-Length", str(len(PIXEL)))
            self.end_headers()
            self.wfile.write(PIXEL)
            return
        super().do_GET()

    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        kind, _, name = self.path.strip("/").partition("/")
        if kind == "report":
            self.reports[name] = json.loads(body)
            self.reported[name].set()
        elif kind == "upload":
            self.uploads[name] = body
        self.send_response(204)
        self.end_headers()

    def end_headers(self):
        desktop = self.path.startswith("/desktop/")
        self.send_header("Content-Security-Policy", self.desktop_policy if desktop else self.policy)
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def log_message(self, *args):
        pass


def prepare_site(dist: Path, site: Path, tauri_stub: bool = False) -> None:
    """Copies a built site and adds the fixtures and the driver script."""
    shutil.copytree(dist, site)
    for fixture in (ROOT / "fixtures").glob("*.xml"):
        shutil.copy(fixture, site / fixture.name)
    shutil.copy(ROOT / "scripts/ui-check-driver.js", site / "ui-check-driver.js")

    invoice = (ROOT / "fixtures/factura.xml").read_bytes()
    with zipfile.ZipFile(site / "many.zip", "w") as archive:
        for index in range(103):
            archive.writestr(f"comprobante-{index + 1:03}.xml", invoice)

    index = site / "index.html"
    marker = '<script type="module" src="app.js"></script>'
    html = index.read_text(encoding="utf-8")
    if marker not in html:
        sys.exit("index.html does not load app.js as expected")
    before = ""
    if tauri_stub:
        # The bridge must exist before the app's modules run.
        shutil.copy(ROOT / "scripts/ui-check-tauri-stub.js", site / "ui-check-tauri-stub.js")
        before = '<script src="ui-check-tauri-stub.js"></script>\n    '
    index.write_text(
        html.replace(
            marker,
            before + marker + '\n    <script type="module" src="ui-check-driver.js"></script>',
        ),
        encoding="utf-8",
    )


def expect(failures: list, scenario: str, label: str, actual, expected) -> None:
    if actual != expected:
        failures.append(f"{scenario}: {label}: expected {expected!r}, got {actual!r}")


def verify_desktop(failures: list, desktop: dict) -> None:
    check = partial(expect, failures, "desktop")
    check("commands at startup", desktop["startupCommands"], ["snapshot", "apply_settings"])
    check("printers", desktop["printers"], ["Predeterminada del sistema", "Caja", "Oficina"])
    check("watched folder", desktop["watchDir"], "/descargas")
    check("output folder", desktop["outputDir"], "/descargas › pdf")
    check("save hint mentions the system dialog", "diálogo del sistema" in desktop["saveHint"], True)
    check(
        "save",
        desktop["save"],
        {
            "status": "Guardado: /guardados/20123456789-01-F001-00000123-a4.pdf",
            "name": "20123456789-01-F001-00000123-a4.pdf",
            "extension": "pdf",
            "header": "%PDF-",
        },
    )
    check(
        "print",
        desktop["print"],
        {
            "status": "Enviado a la impresora: Caja",
            "name": "20123456789-01-F001-00000123-ticket80.pdf",
            "format": "ticket80",
            "width": 80,
            "taller": True,
            "header": "%PDF-",
        },
    )
    check(
        "settings sent to the backend",
        desktop["applied"],
        {
            "watch": {
                "watch_dir": "/descargas",
                "output_dir": None,
                "print": False,
                "printer": "Caja",
                "copies": 3,
                "watch_on_start": False,
                "format": "a4",
            },
            "render": {
                "issuer_lines": [],
                "footer": None,
                "show_qr": True,
                "logo": None,
                "template": None,
                "page_design": None,
                "ticket_design": None,
            },
        },
    )
    check("watch state", desktop["watchState"], "Vigilando")
    check("start_watching sent", desktop["startedWatching"], True)
    check(
        "log, newest first",
        desktop["log"],
        ["roto.xml: not valid XML", "20123456789-01-F001-1.pdf — PDF creado e impreso"],
    )


def verify_designer(failures: list, designer: dict) -> None:
    check = partial(expect, failures, "designer")
    classic = [
        ["issuer", "document"], ["customer", "dates"], ["affected"], ["items"], ["totals"],
        ["words"], ["details"], ["legal", "qr"], ["credit"],
    ]
    check(
        "a handle over every visible block",
        designer["initialHandles"],
        ["r0b0", "r0b1", "r1b0", "r1b1", "r3b0", "r4b0", "r5b0", "r6b0", "r7b0", "r7b1", "r8b0"],
    )
    check("handle labels", designer["initialLabels"], ["Logo y emisor", "RUC y número"])
    check("no design stored before editing", designer["designBeforeEditing"], None)
    check("block editor hidden until a block is chosen", designer["blockEditorHidden"], True)
    check("handles have their place before the image loads", designer["roomReserved"], True)

    check("drop zone beside", designer["zoneBeside"], "left")
    beside = [["issuer", "qr", "document"]] + classic[1:7] + [["legal"], ["credit"]]
    check("block dropped beside another", designer["afterBeside"], beside)
    check("drop zone above", designer["zoneAbove"], "above")
    above = [beside[0], ["totals"], ["customer", "dates"], ["affected"], ["items"], ["words"], ["details"], ["legal"], ["credit"]]
    check("block dropped above another", designer["afterAbove"], above)
    check("a full row sends the block to its own row", designer["afterFullRow"], [beside[0], ["credit"]])
    check("the moved block stays selected", designer["selectedAfterDrag"], "r1b0")

    check(
        "preset",
        designer["preset"],
        {
            "style": {"text": "normal", "boxes": "lines", "heading": "inverted", "stripes": False, "filler_rows": False},
            "firstRow": ["issuer", "document"],
        },
    )
    check(
        "block editor",
        designer["blockEditor"],
        {"hidden": False, "name": "RUC y número", "textField": False, "columnsField": False},
    )
    check(
        "edited block",
        designer["editedBlock"],
        {"type": "document", "width": 4, "boxed": False, "align": "right"},
    )
    check("added text block", designer["addedText"], [{"type": "text", "text": "Gracias por su compra"}])
    check(
        "items columns",
        designer["columns"],
        ["quantity", "unit_code", "description", "unit_value", "unit_price", "total"],
    )
    check("box style", designer["boxes"], "square")
    check("the preview drew every edit", designer["previewError"], "")
    check("bad import reported", designer["importError"].startswith("No se pudo importar el diseño: "), True)
    check("design kept after a bad import", designer["designKeptAfterBadImport"], True)
    check(
        "ticket designer",
        designer["ticket"],
        {"canvasIsTicket": True, "handles": 16, "designBeforeEditing": None, "lastRow": ["spacer"]},
    )
    check("page design kept while editing the ticket", designer["pageDesignUntouched"], True)
    check("conversion with custom designs", designer["converted"], {"formats": 5, "errors": 0})
    check("reset", designer["afterReset"], {"design": None, "handles": 11, "blockEditorHidden": True})


def verify(reports: dict, uploads: dict, scenarios: list) -> list:
    failures: list = []
    for scenario in scenarios:
        report = reports.get(scenario)
        if report is None:
            failures.append(f"{scenario}: the page did not report")
        elif "error" in report:
            failures.append(f"{scenario}: {report['error']} (status: {report.get('status')})")
            if report.get("partial"):
                failures.append(f"{scenario}: gathered before failing: {json.dumps(report['partial'], ensure_ascii=False)}")
    if failures:
        return failures

    check = partial(expect, failures, "paste")
    paste = reports["paste"]
    check("Convertir is disabled without input", paste["disabledWhenEmpty"], True)
    check("input left of the headline", paste["inputLeftOfHeadline"], True)
    check(
        "search metadata",
        paste["seo"],
        {
            "title": "Convertir XML de SUNAT a PDF gratis | xml2pdf · Qlab by Qodi",
            "language": "es-PE",
            "description": True,
            # No public address was given to this build, so none is published.
            "canonical": None,
            "structured": ["WebApplication", "FAQPage"],
            "questions": 6,
            "answersInStructuredData": 6,
        },
    )
    check("status", paste["status"], "Descargado: 20123456789-01-F001-00000123-a4.pdf")
    check("title", paste["title"], "Elige tu formato")
    check("formats", paste["formats"], ["A4", "A5", "Carta", "Ticket 80", "Ticket 58"])
    check("batch list hidden", paste["batchHidden"], True)
    check("errors", paste["errors"], 0)
    check("saved file", paste["saved"], "20123456789-01-F001-00000123-a4.pdf")
    check("saved content", paste["savedHeader"], "%PDF-")

    check = partial(expect, failures, "invalid")
    invalid = reports["invalid"]
    check("status", invalid["status"], "No se pudo convertir. Revisa el detalle más abajo.")
    check(
        "errors",
        invalid["errors"],
        ["pegado.xml: unsupported document `ApplicationResponse`: expected Invoice, CreditNote or DebitNote"],
    )
    check("formats", invalid["formats"], 0)

    check = partial(expect, failures, "bulk")
    bulk = reports["bulk"]
    check(
        "limit notice when adding",
        bulk["addStatus"],
        "Se agregaron 100 archivos; 5 quedaron fuera por el límite de 100 comprobantes o 50 MB por tanda.",
    )
    check("title", bulk["title"], "99 comprobantes")
    check("rows", bulk["rows"], 100)
    check(
        "failed rows",
        bulk["failedRows"],
        ["lote-003.xmlunsupported document `ApplicationResponse`: expected Invoice, CreditNote or DebitNote"],
    )
    check("first document selected", bulk["selectedRow"], 0)
    check("formats", bulk["formats"], 5)
    check("omitted notice hidden", bulk["noticeHidden"], True)
    check("selection after click", bulk["selectedAfterClick"], 1)
    check("Guardar todos disabled while running", bulk["saveAllDisabledWhileRunning"], True)
    check("bulk status", bulk["bulkStatus"], "99 PDF en el ZIP.")
    check("zip name", bulk["zipName"], "comprobantes-ticket80.zip")
    with zipfile.ZipFile(io.BytesIO(uploads.get("batch.zip", b""))) as archive:
        names = archive.namelist()
        check("files in the ZIP", len(names), 99)
        check("unique names in the ZIP", len(set(names)), 99)
        check("every file is a PDF", all(archive.read(name).startswith(b"%PDF-") for name in names), True)

    check = partial(expect, failures, "ziplimit")
    ziplimit = reports["ziplimit"]
    check("rows", ziplimit["rows"], 100)
    check("title", ziplimit["title"], "100 comprobantes")
    check("notice visible", ziplimit["noticeHidden"], False)
    check(
        "notice",
        ziplimit["notice"],
        "Se convirtieron los primeros 100 comprobantes; 3 quedaron fuera de esta tanda. "
        "Conviértelos en una tanda aparte.",
    )
    verify_designer(failures, reports["designer"])
    if "desktop" in scenarios:
        verify_desktop(failures, reports["desktop"])
    return failures


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("dist", nargs="?", default=str(ROOT / "dist/web"), help="built site folder")
    parser.add_argument("--desktop", default=str(ROOT / "dist/desktop"), help="built desktop interface folder")
    parser.add_argument("--screenshots", help="folder to keep one screenshot per scenario")
    arguments = parser.parse_args()

    dist = Path(arguments.dist)
    if not (dist / "pkg/xml2pdf_wasm_bg.wasm").is_file():
        sys.exit(f"{dist} is not a built site; run scripts/build-web.sh first")
    firefox = shutil.which("firefox")
    if not firefox:
        sys.exit("Firefox is required")

    with tempfile.TemporaryDirectory(prefix="xml2pdf-ui-check-") as scratch:
        scratch_dir = Path(scratch)
        prepare_site(dist, scratch_dir / "site")
        scenarios = list(SCENARIOS)
        if (Path(arguments.desktop) / "panel.html").is_file():
            prepare_site(Path(arguments.desktop), scratch_dir / "site/desktop", tauri_stub=True)
            scenarios.append("desktop")
        else:
            print("skipped desktop (interface not built)")
        shots = Path(arguments.screenshots) if arguments.screenshots else scratch_dir / "shots"
        shots.mkdir(parents=True, exist_ok=True)

        Handler.policy = content_security_policy()
        Handler.desktop_policy = desktop_security_policy()
        Handler.reported = {scenario: threading.Event() for scenario in scenarios}
        server = ThreadingHTTPServer(("127.0.0.1", 0), partial(Handler, directory=str(scratch_dir / "site")))
        threading.Thread(target=server.serve_forever, daemon=True).start()
        origin = f"http://127.0.0.1:{server.server_address[1]}"

        for scenario in scenarios:
            # The image request keeps the page loading until the scenario reports.
            folder = "desktop/" if scenario == "desktop" else ""
            page = scratch_dir / "site" / folder / f"{scenario}.html"
            html = (page.parent / "index.html").read_text(encoding="utf-8")
            page.write_text(
                html.replace("</body>", f'<img src="/wait/{scenario}" alt="" width="1" height="1" /></body>'),
                encoding="utf-8",
            )
            profile = scratch_dir / f"profile-{scenario}"
            profile.mkdir()
            try:
                subprocess.run(
                    [
                        firefox, "--headless", "--no-remote",
                        "--profile", str(profile),
                        "--window-size=1440,2300",
                        "--screenshot", str(shots / f"{scenario}.png"),
                        f"{origin}/{folder}{scenario}.html?scenario={scenario}",
                    ],
                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                    timeout=SCENARIO_TIMEOUT_SECONDS + 30, check=False,
                )
            except subprocess.TimeoutExpired:
                pass
            print(f"{'ok ' if scenario in Handler.reports else 'NO REPORT'} {scenario}")
        server.shutdown()

        failures = verify(Handler.reports, Handler.uploads, scenarios)

    for failure in failures:
        print(f"FAIL {failure}")
    print("ui check passed" if not failures else f"ui check failed: {len(failures)} problem(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
