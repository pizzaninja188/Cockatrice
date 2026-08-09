#!/usr/bin/env bash
# Generates vanilla / french-vanilla card RON from a Scryfall bulk dump (Phase 6).
#
# Writes one RON file per qualifying card under tricerules-cards/data/generated/<letter>/;
# build.rs embeds them automatically. Normal creatures and supported two-face layouts are
# eligible. Cards already present in data/ (by id or name) and cards with rules text beyond
# the supported keyword set are skipped.
#
# Workflow:
#   ./scripts/fetch-scryfall-bulk.sh                 # download oracle-cards.json
#   ./scripts/gen-cards.sh --dry-run                 # preview counts + skip reasons
#   ./scripts/gen-cards.sh                           # write the RON files
#   (cd tricerules && cargo test)                    # registry + conformance validate every card
#   ./scripts/gen-card-checklist.sh --check          # name gate, then review + commit
#
# Any extra args pass through to gen-cards (e.g. --limit 50, --out-dir <path>).
# The default --input is oracle-cards.json in the repo root; override with --input <path>.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$REPO_ROOT/tricerules/Cargo.toml"
DEFAULT_INPUT="$REPO_ROOT/oracle-cards.json"

# Inject a default --input if the caller didn't supply one.
has_input=0
for arg in "$@"; do
    [[ "$arg" == "--input" ]] && has_input=1 && break
done

extra_args=()
if [[ $has_input -eq 0 ]]; then
    if [[ ! -f "$DEFAULT_INPUT" ]]; then
        echo "error: $DEFAULT_INPUT not found — run ./scripts/fetch-scryfall-bulk.sh first," >&2
        echo "       or pass --input <path>." >&2
        exit 1
    fi
    extra_args=(--input "$DEFAULT_INPUT")
fi

cargo run --release --manifest-path "$MANIFEST" \
    -p tricerules-cards --features gencards --bin gen-cards \
    -- "${extra_args[@]}" "$@"
