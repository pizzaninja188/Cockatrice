#!/usr/bin/env bash
# Regenerates tricerules/CARDS.md, the implemented-card checklist grouped by set.
#
# Implementation status comes from the tricerules card registry; set grouping and
# release dates come from the Oracle cards.xml. Each card is filed under its
# first/original printing.
#
# Usage:
#   ./scripts/gen-card-checklist.sh
#   ./scripts/gen-card-checklist.sh --sets ZNR,DOM
#   ./scripts/gen-card-checklist.sh --cards-xml /path/to/cards.xml
#
# Options (passed through to gen-checklist):
#   --sets ZNR,DOM        only emit those sets
#   --ignore-online-only  skip online-only printings when picking the first printing
#   --include-tokens      include token cards
#   --check               exit nonzero if any implemented card name has no Oracle match
#   --cards-xml <path>    use a specific cards.xml
#                         (default: $XDG_DATA_HOME/Cockatrice/Cockatrice/cards.xml
#                                or ~/.local/share/Cockatrice/Cockatrice/cards.xml)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$REPO_ROOT/tricerules/Cargo.toml"
OUT="$REPO_ROOT/tricerules/CARDS.md"

# Inject a Linux-default --cards-xml if the caller didn't supply one
has_cards_xml=0
for arg in "$@"; do
    [[ "$arg" == "--cards-xml" ]] && has_cards_xml=1 && break
done

extra_args=()
if [[ $has_cards_xml -eq 0 ]]; then
    xdg_data="${XDG_DATA_HOME:-$HOME/.local/share}"
    default_xml="$xdg_data/Cockatrice/Cockatrice/cards.xml"
    if [[ -f "$default_xml" ]]; then
        extra_args=(--cards-xml "$default_xml")
    fi
fi

cargo run --release --manifest-path "$MANIFEST" \
    -p tricerules-cards --features checklist --bin gen-checklist \
    -- --out "$OUT" "${extra_args[@]}" "$@"
