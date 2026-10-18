#!/usr/bin/env bash
# mammoth on the corpus, iterations from bench/plan.toml.
# usage: run.sh CDOC_BENCH PLAN CORPUS_DIR PAGES(1,10,..) OUT_DIR
set -euo pipefail
bin=$1 plan=$2 corpus=$3 pages=$4 out=$5
here=$(cd "$(dirname "$0")" && pwd)
(cd "$here" && npm ci --ignore-scripts --no-audit --no-fund)
runs=()
for p in ${pages//,/ }; do
  read -r warmup iterations < <("$bin" plan --plan "$plan" --key "mammoth/docx_open/${p}p")
  runs+=("$p:$warmup:$iterations")
done
mkdir -p "$out"
raw=$(mktemp)
node "$here/run.mjs" "$raw" "$corpus" "${runs[@]}"
"$bin" summarize --raw "$raw" --cells any --out "$out/mammoth.json"
rm -f "$raw"
