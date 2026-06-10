<#
.SYNOPSIS
    Regenerates tricerules/CARDS.md, the implemented-card checklist grouped by set.
.DESCRIPTION
    Implementation status (full / partial / not started) comes from the tricerules card
    registry; set grouping and release dates come from the Oracle cards.xml. Each card is
    filed under its first/original printing.

    Any extra arguments are passed straight through to the generator, e.g.:
        --sets ZNR,DOM        only emit those sets
        --ignore-online-only  skip online-only printings when picking the first printing
        --include-tokens      include token cards
        --check               exit nonzero if any implemented card name has no Oracle match
        --cards-xml <path>    use a specific cards.xml (default: %LOCALAPPDATA%\Cockatrice\Cockatrice\cards.xml)
.EXAMPLE
    ./scripts/gen-card-checklist.ps1
.EXAMPLE
    ./scripts/gen-card-checklist.ps1 --sets ZNR,DOM
.EXAMPLE
    ./scripts/gen-card-checklist.ps1 --check
#>

$repoRoot = Split-Path -Parent $PSScriptRoot
$manifest = Join-Path $repoRoot "tricerules\Cargo.toml"
$out = Join-Path $repoRoot "tricerules\CARDS.md"

cargo run --release --manifest-path $manifest -p tricerules-cards --features checklist --bin gen-checklist -- --out $out @args
