#!/usr/bin/env bash
# One product through the browser protocol (README.md, wave 3), iterations from bench/plan.toml.
# Expects the driver built (npm ci && npm run build in bench/competitors) and Playwright's Chromium
# installed. A product that needs a test account and has none writes "not measured" cells.
# usage: run.sh PRODUCT CDOC_BENCH PLAN CORPUS_DIR OUT_DIR ARTIFACT_DIR
#   env: MACHINE (the runner, as scripts/runner-machine.sh describes it), COMMIT, RUN_URL
set -euo pipefail
product=$1 bin=$2 plan=$3 corpus=$4 out=$5 artifacts=$6
: "${MACHINE:?}" "${COMMIT:?}" "${RUN_URL:?}"
root=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$out" "$artifacts"
"$bin" browser-plan --plan "$plan" --tool "$product" --corpus "$corpus" \
  --out "$artifacts/$product-plan.json"
xvfb-run -a -s "-screen 0 1920x1080x24" node "$root/dist/browser/main.js" --product "$product" \
  --plan "$artifacts/$product-plan.json" --corpus "$corpus" --out "$artifacts/$product-raw.json" \
  --notes "$artifacts/$product-notes.json" --shots "$artifacts/shots" --runner "$MACHINE" \
  --commit "$COMMIT" --date "$(date -u +%F)" --run-url "$RUN_URL"
# Ours fills the Docs rows and must say which UI and engine it measured; a product must not.
cells=browser
[ "$product" = ours ] && cells=browser-ours
"$bin" summarize --raw "$artifacts/$product-raw.json" --cells "$cells" --out "$out/$product.json"
