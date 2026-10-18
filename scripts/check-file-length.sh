#!/usr/bin/env bash
# Every source file is at most 250 lines. Larger files are split, not exempted.
set -euo pipefail
cd "$(dirname "$0")/.."
LIMIT=250
status=0
while IFS= read -r file; do
  lines=$(wc -l < "$file")
  if [ "$lines" -gt "$LIMIT" ]; then
    echo "$file: $lines lines (limit $LIMIT)" >&2
    status=1
  fi
done < <(git ls-files 'crates/**/*.rs' 'scripts/*.sh' '.github/**/*.yml' \
  'bench/**/*.sh' 'bench/**/*.mjs' 'bench/**/*.py' 'bench/**/*.ts' 'bench/**/*.html')
[ "$status" -eq 0 ] && echo "file lengths OK"
exit $status
