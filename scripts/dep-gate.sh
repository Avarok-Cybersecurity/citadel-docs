#!/usr/bin/env bash
# The dependency gate: the WASM build must contain no C code and no I/O crates.
#
# Pure Rust only, so the artefact runs anywhere wasm32-unknown-unknown runs and the supply chain
# stays auditable. Randomness and the clock are forbidden too: the cores take them through
# ports, never from the environment (SBIO). The shaper is gated on its own until it is wired into
# cdoc-wasm, and no third-party shaper may reach either: rustybuzz is its test oracle only.
# The benchmark harness and the competitors it measures (docx-rs, yrs) never ship.
set -euo pipefail
cd "$(dirname "$0")/.."

DENY='^(ring|openssl|openssl-sys|openssl-src|native-tls|rustls-native-certs|cc|cmake|pkg-config|libz-sys|zlib-sys|libc|getrandom|rand|rand_core|instant|chrono|time|tokio|reqwest|hyper|.*-sys|rustybuzz|harfbuzz.*|cdoc-bench|docx-rs|yrs)$'

# Every crate that ships to the browser: the facade, the shaper, and the CRDT the collaboration
# layer runs.
GATED='cdoc-wasm cdoc-shape cdoc-crdt cdoc-edit'

status=0
for pkg in $GATED; do
  crates="$(cargo tree -p "$pkg" --target wasm32-unknown-unknown -e normal --prefix none \
    | awk '{print $1}' | sort -u)"
  bad="$(printf '%s\n' "$crates" | grep -E "$DENY" || true)"
  if [ -n "$bad" ]; then
    echo "dependency gate FAILED: forbidden crates in the wasm32 build of $pkg:" >&2
    printf '  %s\n' $bad >&2
    status=1
  else
    echo "dependency gate OK for $pkg: $(printf '%s\n' "$crates" | wc -l | tr -d ' ') crates, none forbidden"
  fi
done
exit $status
