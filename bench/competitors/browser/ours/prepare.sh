#!/usr/bin/env bash
# Checks out the pinned UI (ui-pin: a citadel-workspace-ui commit, public, no parent checkout) and
# installs what the harness build needs: ui-deps.js derives the package.json from the UI's own,
# and npm ci installs it exactly as ui.package-lock.json pins it (and refuses it if they drifted).
# Expects the driver built (dist/). usage: prepare.sh UI_DIR
set -euo pipefail
ui=$1
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
pin=$(tr -d '[:space:]' < "$here/ui-pin")
[[ "$pin" =~ ^[0-9a-f]{40}$ ]] || { echo "ui-pin is not a full commit id" >&2; exit 1; }
git init -q "$ui"
git -C "$ui" fetch -q --depth 1 https://github.com/Avarok-Cybersecurity/citadel-workspace-ui "$pin"
git -C "$ui" checkout -q --detach FETCH_HEAD
test "$(git -C "$ui" rev-parse HEAD)" = "$pin"
node "$root/dist/browser/ours/ui-deps.js" "$ui/package.json" "$ui/package.json"
cp "$here/ui.package-lock.json" "$ui/package-lock.json"
(cd "$ui" && npm ci --ignore-scripts --no-audit --no-fund)
