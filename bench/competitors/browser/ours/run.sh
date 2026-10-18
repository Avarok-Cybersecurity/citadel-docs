#!/usr/bin/env bash
# Ours through the browser protocol: the pinned UI's bench harness page (prepare.sh checked it out
# and installed it) built and served cross-origin isolated by its serve.mjs on the runner's
# loopback, then browser/run.sh. The version names the UI commit and the citadel-docs commit its
# vendored cdoc-wasm was built from (vendor/cdoc/PROVENANCE.json); both go into the provenance.
# usage: run.sh CDOC_BENCH PLAN CORPUS_DIR OUT_DIR ARTIFACT_DIR UI_DIR   (env as browser/run.sh)
set -euo pipefail
bin=$1 plan=$2 corpus=$3 out=$4 artifacts=$5 ui=$6
here=$(cd "$(dirname "$0")" && pwd)
port=4791
mkdir -p "$artifacts"
ui_commit=$(git -C "$ui" rev-parse HEAD)
test "$ui_commit" = "$(tr -d '[:space:]' < "$here/ui-pin")"
engine_commit=$(node -e 'process.stdout.write(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).citadel_docs_commit)' \
  "$ui/vendor/cdoc/PROVENANCE.json")
build=$(mktemp -d)
cleanup() { kill "${server_pid:-0}" 2>/dev/null || true; rm -rf "$build"; }
trap cleanup EXIT
(cd "$ui" && exec node scripts/lib/bench-harness/serve.mjs --port "$port" --out "$build") \
  > "$artifacts/ours-harness.log" 2>&1 &
server_pid=$!
for _ in $(seq 1 120); do
  curl -fsS "http://127.0.0.1:$port/" -o /dev/null 2>/dev/null && break
  kill -0 "$server_pid" 2>/dev/null || { cat "$artifacts/ours-harness.log" >&2; exit 1; }
  sleep 2
done
# The page must be cross-origin isolated, or memory is not measured.
curl -fsSI "http://127.0.0.1:$port/" | grep -qi '^cross-origin-embedder-policy: require-corp'
CDOC_HARNESS_URL="http://127.0.0.1:$port/" CDOC_UI_COMMIT="$ui_commit" CDOC_ENGINE_COMMIT="$engine_commit" \
  CDOC_VERSION="UI ${ui_commit:0:12}, cdoc-wasm ${engine_commit:0:12}" \
  "$here/../run.sh" ours "$bin" "$plan" "$corpus" "$out" "$artifacts"
