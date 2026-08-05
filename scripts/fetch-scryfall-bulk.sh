#!/usr/bin/env bash
# Downloads the Scryfall "oracle_cards" bulk data file (one representative printing per
# card) — a single JSON download, no per-card API calls and no rate-limit exposure.
#
# Scryfall is the authoritative card-data source per AGENTS.md. A descriptive User-Agent
# is required by their API guidelines.
#
# Usage:
#   ./scripts/fetch-scryfall-bulk.sh                 # -> oracle-cards.json in the repo root
#   ./scripts/fetch-scryfall-bulk.sh /tmp/oracle.json

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${1:-$REPO_ROOT/oracle-cards.json}"
UA="Cockatrice-tricerules-gencards/1.0 (https://github.com/Cockatrice/Cockatrice)"

command -v curl >/dev/null || { echo "error: curl is required" >&2; exit 1; }
command -v jq >/dev/null || { echo "error: jq is required" >&2; exit 1; }

echo "Querying Scryfall bulk-data index..." >&2
download_uri="$(curl -fsSL -A "$UA" https://api.scryfall.com/bulk-data \
    | jq -r '.data[] | select(.type == "oracle_cards") | .download_uri')"

if [[ -z "$download_uri" || "$download_uri" == "null" ]]; then
    echo "error: could not find the oracle_cards download URI in the bulk-data index" >&2
    exit 1
fi

echo "Downloading $download_uri" >&2
curl -fSL -A "$UA" -o "$OUT" "$download_uri"
echo "Wrote $OUT ($(du -h "$OUT" | cut -f1))." >&2
