#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DB="${1:-$HOME/.local/share/vide/vide.db}"
JSON="${2:-$HOME/Documents/Archive/lx_list}"

if [[ ! -f "$DB" ]]; then
  echo "database not found: $DB" >&2
  exit 1
fi
if [[ ! -f "$JSON" ]]; then
  echo "LX list not found: $JSON" >&2
  exit 1
fi

echo "database: $DB"
echo "json:     $JSON"
cargo run --quiet --manifest-path "$ROOT/src-tauri/Cargo.toml" --bin import_lx -- "$JSON" "$DB"
