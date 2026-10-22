#!/usr/bin/env bash
# Builds every artefact in crates/cdoc-wasm/artefacts.txt the way the UI ships it, into OUT:
#   cargo (release, wasm32-unknown-unknown, --locked) -> wasm-bindgen --target web -> wasm-opt -Oz
# giving OUT/cdoc_wasm_<name>_bg.wasm and its glue OUT/cdoc_wasm_<name>.js / .d.ts, and gates each:
#   - the raw module imports only wasm-bindgen's own glue modules (no WASI, no env: no host I/O);
#   - the shipped module's exports are exactly crates/cdoc-wasm/surface/<name>.exports, so a
#     feature leaking into an artefact (or an export going missing) is red;
#   - an artefact that migrates chat documents does so in the shipped module under any non-zero
#     u64 client (scripts/check-wasm-migrate.mjs);
#   - the default build ships as `full`: the same glue and the same exports. Not the same bytes:
#     symbol hashes take the feature list, `default` included, and they order functions.
# Prints `<name> <raw> <opt> <gzip> <brotli>` bytes per artefact to OUT/sizes.txt.
#
#   CARGO_TARGET_DIR=<dir> scripts/build-wasm.sh OUT [--bless]
# --bless rewrites the export snapshots instead of checking them (review the diff).
set -euo pipefail
cd "$(dirname "$0")/.."

die() { echo "build-wasm: $*" >&2; exit 1; }
OUT="${1:-}"
[ -n "$OUT" ] || die "usage: CARGO_TARGET_DIR=<dir> $0 OUT [--bless]"
BLESS="${2:-}"
[ -z "$BLESS" ] || [ "$BLESS" = "--bless" ] || die "unknown flag $BLESS"
[ -n "${CARGO_TARGET_DIR:-}" ] || die "set CARGO_TARGET_DIR"
for tool in wasm-bindgen wasm-opt node gzip brotli; do
  command -v "$tool" >/dev/null || die "$tool not found"
done
LOCK_BINDGEN="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)"
CLI_BINDGEN="$(wasm-bindgen --version | awk '{ print $2 }')"
[ "$LOCK_BINDGEN" = "$CLI_BINDGEN" ] || die "wasm-bindgen CLI $CLI_BINDGEN is not the crate $LOCK_BINDGEN"

RAW="$CARGO_TARGET_DIR/wasm32-unknown-unknown/release/cdoc_wasm.wasm"
SURFACE=crates/cdoc-wasm/surface
build() { cargo build --locked --release --target wasm32-unknown-unknown -p cdoc-wasm "$@"; }
size() { wc -c < "$1" | tr -d ' '; }
# ship DIR NAME: the raw module through wasm-bindgen and wasm-opt into DIR/cdoc_wasm_NAME_bg.wasm.
ship() {
  wasm-bindgen --target web --out-dir "$1" --out-name "cdoc_wasm_$2" "$RAW"
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    --enable-mutable-globals --enable-reference-types --enable-multivalue \
    "$1/cdoc_wasm_$2_bg.wasm" -o "$1/cdoc_wasm_$2_bg.wasm"
}
mkdir -p "$OUT"
: > "$OUT/sizes.txt"
status=0

while read -r name feature; do
  case "$name" in ''|'#'*) continue ;; esac
  build --no-default-features --features "$feature"
  cp "$RAW" "$OUT/$name.raw.wasm"
  unexpected="$(node scripts/wasm-surface.mjs "$RAW" | sed -n 's/^import //p' \
    | grep -vE '^__wbindgen_(placeholder|externref_xform)__$' || true)"
  if [ -n "$unexpected" ]; then
    echo "$name imports from the host: $unexpected" >&2; status=1
  fi
  ship "$OUT" "$name"
  shipped="$OUT/cdoc_wasm_${name}_bg.wasm"
  node scripts/wasm-surface.mjs "$shipped" | sed -n 's/^export //p' > "$OUT/$name.exports"
  if [ "$BLESS" = "--bless" ]; then
    cp "$OUT/$name.exports" "$SURFACE/$name.exports"
  elif ! diff -u "$SURFACE/$name.exports" "$OUT/$name.exports"; then
    echo "$name exports are not $SURFACE/$name.exports" >&2; status=1
  fi
  if grep -qx tiptap_to_crdt_snapshot "$OUT/$name.exports"; then
    node scripts/check-wasm-migrate.mjs "$OUT" "$name" || status=1
  fi
  echo "$name $(size "$OUT/$name.raw.wasm") $(size "$shipped") $(gzip -9 -c "$shipped" | wc -c | tr -d ' ') $(brotli -q 11 -c "$shipped" | wc -c | tr -d ' ')" | tee -a "$OUT/sizes.txt"
done < crates/cdoc-wasm/artefacts.txt

build
ship "$OUT/default" full
cmp "$OUT/default/cdoc_wasm_full.js" "$OUT/cdoc_wasm_full.js" \
  || { echo "the default build's glue is not full's" >&2; status=1; }
node scripts/wasm-surface.mjs "$OUT/default/cdoc_wasm_full_bg.wasm" | sed -n 's/^export //p' \
  | diff -u "$SURFACE/full.exports" - || { echo "the default build's exports are not full's" >&2; status=1; }
exit $status
