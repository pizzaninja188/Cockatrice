#!/usr/bin/env bash
# Downloads Scryfall gzipped JSONL bulk data with verified provenance sidecars.
#
# Scryfall is the authoritative card-data source per AGENTS.md. A descriptive User-Agent
# is required by their API guidelines.
#
# Usage:
#   ./scripts/fetch-scryfall-bulk.sh                 # -> oracle-cards.jsonl.gz
#   ./scripts/fetch-scryfall-bulk.sh /tmp/oracle-cards.jsonl.gz
#   ./scripts/fetch-scryfall-bulk.sh /tmp/oracle-cards.jsonl.gz --include-oracle-tags

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${1:-$REPO_ROOT/oracle-cards.jsonl.gz}"
INCLUDE_TAGS="${2:-}"
TAGS_OUT="$REPO_ROOT/oracle-tags.jsonl.gz"
UA="Cockatrice-tricerules-gencards/1.0 (https://github.com/Cockatrice/Cockatrice)"

command -v curl >/dev/null || { echo "error: curl is required" >&2; exit 1; }
command -v jq >/dev/null || { echo "error: jq is required" >&2; exit 1; }

echo "Querying Scryfall bulk-data index..." >&2
index="$(curl -fsSL -A "$UA" -H 'Accept: application/json' https://api.scryfall.com/bulk-data)"

download_bulk() {
    local type="$1"
    local out="$2"
    local entry uri sha
    entry="$(printf '%s' "$index" | jq -c --arg type "$type" '.data[] | select(.type == $type)')"
    uri="$(printf '%s' "$entry" | jq -r '.jsonl_download_uri')"
    if [[ -z "$uri" || "$uri" == "null" ]]; then
        echo "error: could not find the $type jsonl_download_uri in the bulk-data index" >&2
        exit 1
    fi

    echo "Downloading $uri" >&2
    curl -fSL -A "$UA" -H 'Accept: application/gzip' -o "$out" "$uri"
    if command -v sha256sum >/dev/null; then
        sha="$(sha256sum "$out" | cut -d' ' -f1)"
    else
        sha="$(shasum -a 256 "$out" | cut -d' ' -f1)"
    fi
    printf '%s' "$entry" | jq --arg sha256 "$sha" \
        '{type, id, updated_at, jsonl_download_uri, sha256: $sha256}' > "$out.meta.json"
    echo "Wrote $out ($(du -h "$out" | cut -f1)); sha256:$sha" >&2
}

download_bulk oracle_cards "$OUT"
if [[ "$INCLUDE_TAGS" == "--include-oracle-tags" ]]; then
    download_bulk oracle_tags "$TAGS_OUT"
fi
