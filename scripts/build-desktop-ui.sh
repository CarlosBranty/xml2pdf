#!/usr/bin/env sh
# Builds the interface of the desktop app into `dist/desktop`: the web app
# plus the desktop platform layer (system dialogs, printing, folder watcher).
#
# Same requirements as scripts/build-web.sh. Run it before building the
# desktop app, which embeds this folder.
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
out="${1:-$root/dist/desktop}"

"$root/scripts/build-web.sh" "$out"
cp "$root/apps/desktop/ui/platform.js" "$root/apps/desktop/ui/panel.html" "$root/apps/desktop/ui/desktop.css" "$out/"

echo "Desktop interface built in $out"
