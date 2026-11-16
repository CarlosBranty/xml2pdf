#!/usr/bin/env python3
"""End-to-end check of the web app in a real browser.

Serves a copy of the built site with the Content-Security-Policy of
deploy/Caddyfile, opens it in headless Firefox once per scenario and verifies
what the page reported: pasting an XML, an invalid document, a 105-file batch
with bulk saving, and a ZIP over the batch limit.

Usage: python3 scripts/ui-check.py [dist/web] [--screenshots DIR]
Requires Firefox. Build the site first with scripts/build-web.sh.
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
SCENARIOS = ["paste", "invalid", "bulk", "ziplimit"]
SCENARIO_TIMEOUT_SECONDS = 240
# A 1x1 transparent PNG, served once a scenario has reported.
PIXEL = bytes.fromhex(
    "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c489"
    "0000000d49444154789c6360000002000001e221bc330000000049454e44ae426082"
)


def content_security_policy() -> str:
    caddyfile = (ROOT / "deploy/Caddyfile").read_text(encoding="utf-8")
    match = re.search(r'Content-Security-Policy "([^"]+)"', caddyfile)
    if not match:
        sys.exit("deploy/Caddyfile has no Content-Security-Policy")
    return match.group(1)


class Handler(SimpleHTTPRequestHandler):
    """Static files plus the endpoints the driver script talks to."""

    policy = ""
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
        self.send_header("Content-Security-Policy", self.policy)
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def log_message(self, *args):
        pass


def prepare_site(dist: Path, site: Path) -> None:
    """Copies the built site and adds the fixtures and the driver script."""
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
    index.write_text(
        html.replace(marker, marker + '\n    <script type="module" src="ui-check-driver.js"></script>'),
        encoding="utf-8",
    )


def expect(failures: list, scenario: str, label: str, actual, expected) -> None:
    if actual != expected:
        failures.append(f"{scenario}: {label}: expected {expected!r}, got {actual!r}")


def verify(reports: dict, uploads: dict) -> list:
    failures: list = []
    for scenario in SCENARIOS:
        report = reports.get(scenario)
        if report is None:
            failures.append(f"{scenario}: the page did not report")
        elif "error" in report:
            failures.append(f"{scenario}: {report['error']} (status: {report.get('status')})")
    if failures:
        return failures

    check = partial(expect, failures, "paste")
    paste = reports["paste"]
    check("Convertir is disabled without input", paste["disabledWhenEmpty"], True)
    check("status", paste["status"], "Descargado: 20123456789-01-F001-00000123-a4.pdf")
    check("title", paste["title"], "Elige tu formato")
    check("formats", paste["formats"], ["A4", "A5", "Ticket 80", "Ticket 58"])
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
    check("formats", bulk["formats"], 4)
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
    return failures


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("dist", nargs="?", default=str(ROOT / "dist/web"), help="built site folder")
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
        shots = Path(arguments.screenshots) if arguments.screenshots else scratch_dir / "shots"
        shots.mkdir(parents=True, exist_ok=True)

        Handler.policy = content_security_policy()
        Handler.reported = {scenario: threading.Event() for scenario in SCENARIOS}
        server = ThreadingHTTPServer(("127.0.0.1", 0), partial(Handler, directory=str(scratch_dir / "site")))
        threading.Thread(target=server.serve_forever, daemon=True).start()
        origin = f"http://127.0.0.1:{server.server_address[1]}"

        for scenario in SCENARIOS:
            # The image request keeps the page loading until the scenario reports.
            page = scratch_dir / "site" / f"{scenario}.html"
            html = (scratch_dir / "site/index.html").read_text(encoding="utf-8")
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
                        "--window-size=1440,1700",
                        "--screenshot", str(shots / f"{scenario}.png"),
                        f"{origin}/{scenario}.html?scenario={scenario}",
                    ],
                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                    timeout=SCENARIO_TIMEOUT_SECONDS + 30, check=False,
                )
            except subprocess.TimeoutExpired:
                pass
            print(f"{'ok ' if scenario in Handler.reports else 'NO REPORT'} {scenario}")
        server.shutdown()

        failures = verify(Handler.reports, Handler.uploads)

    for failure in failures:
        print(f"FAIL {failure}")
    print("ui check passed" if not failures else f"ui check failed: {len(failures)} problem(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
