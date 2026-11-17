// Stand-in for the Tauri bridge, used by scripts/ui-check.py to run the
// desktop interface in a browser. It records every command the page sends
// and answers like the real backend would.
(() => {
  const calls = [];
  const listeners = {};
  const snapshot = {
    config: {
      watch_dir: "/descargas",
      output_dir: null,
      print: false,
      printer: "Caja",
      copies: 1,
      watch_on_start: false,
      settings: { format: "a4", issuer_lines: [], footer: null, show_qr: true },
      logo: null,
      template: null,
    },
    watching: false,
    printers: ["Caja", "Oficina"],
    printer_error: null,
  };
  const replies = {
    snapshot: () => snapshot,
    apply_settings: () => null,
    start_watching: () => null,
    stop_watching: () => null,
    pick_folder: () => "/elegida",
    save_file: (args) => `/guardados/${args.name}`,
    print_pdf: () => "Caja",
  };

  window.__TAURI__ = {
    core: {
      async invoke(command, args = {}) {
        calls.push({ command, args });
        if (!(command in replies)) throw new Error(`unknown command: ${command}`);
        return replies[command](args);
      },
    },
    event: {
      async listen(name, handler) {
        listeners[name] = handler;
        return () => {};
      },
    },
  };
  window.__tauriStub = {
    calls,
    emit: (name, payload) => listeners[name]?.({ payload }),
  };
})();
