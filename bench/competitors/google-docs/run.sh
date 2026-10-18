#!/usr/bin/env bash
# Google Docs through the browser protocol. Runs only with its test account in the
# environment (BENCH_GDOCS_EMAIL, BENCH_GDOCS_PASSWORD); without it, every cell is
# "not measured" with reason "no test account configured".
# usage: run.sh CDOC_BENCH PLAN CORPUS_DIR OUT_DIR ARTIFACT_DIR
set -euo pipefail
exec "$(dirname "$0")/../browser/run.sh" google-docs "$@"
