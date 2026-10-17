#!/usr/bin/env bash
# LibreOffice headless on the corpus, iterations from bench/plan.toml.
#   cold: `soffice --headless --convert-to pdf`, one process launch per run (open + export);
#   warm: one running `soffice --invisible` instance driven over UNO (uno_bench.py), open alone
#         and open + export.
# usage: run.sh CDOC_BENCH PLAN CORPUS_DIR PAGES(1,10,..) OUT_DIR
set -euo pipefail
bin=$1 plan=$2 corpus=$3 pages=$4 out=$5
here=$(cd "$(dirname "$0")" && pwd)
version=$(soffice --version | awk '{print $2}')
test -n "$version"
mkdir -p "$out"
work=$(mktemp -d)
profile="file://$work/profile"
trap 'kill "${office_pid:-0}" 2>/dev/null || true; rm -rf "$work"' EXIT

convert() {  # one cold open + export
  soffice -env:UserInstallation="$profile-cold" --headless --norestore \
    --convert-to pdf --outdir "$work/pdf" "$1" >/dev/null
  test -s "$work/pdf/$(basename "${1%.docx}").pdf"
}

# Cold. The first conversion creates the profile; it is not timed.
convert "$corpus/1p.docx"
samples=""
for p in ${pages//,/ }; do
  read -r warmup iterations < <("$bin" plan --plan "$plan" --key "libreoffice-cold/docx_to_pdf/${p}p")
  for ((k = 0; k < warmup; k++)); do convert "$corpus/${p}p.docx"; done
  times=""
  for ((k = 0; k < iterations; k++)); do
    start=$(date +%s%N)
    convert "$corpus/${p}p.docx"
    times="$times${times:+,}$(($(date +%s%N) - start))"
  done
  samples="$samples${samples:+,}\"docx_to_pdf/${p}p\":[$times]"
done
printf '{"tool":"LibreOffice (cold, process launch)","version":"%s","samples":{%s},"unmeasured":{}}\n' \
  "$version" "$samples" > "$work/cold.json"
"$bin" summarize --raw "$work/cold.json" --out "$out/libreoffice-cold.json"

# Warm.
port=2002
soffice -env:UserInstallation="$profile-warm" --headless --invisible --norestore --nologo \
  --accept="socket,host=127.0.0.1,port=$port;urp;StarOffice.ComponentContext" &
office_pid=$!
runs=()
for p in ${pages//,/ }; do
  read -r warmup iterations < <("$bin" plan --plan "$plan" --key "libreoffice-warm/docx_to_pdf/${p}p")
  runs+=("$p:$warmup:$iterations")
done
python3 "$here/uno_bench.py" "$port" "$version" "$work/warm.json" "$corpus" "${runs[@]}"
"$bin" summarize --raw "$work/warm.json" --out "$out/libreoffice-warm.json"
