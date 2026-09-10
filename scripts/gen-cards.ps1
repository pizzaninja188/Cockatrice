<#
.SYNOPSIS
    Generates exact-recipe card RON from a Scryfall bulk dump.
.DESCRIPTION
    Writes one RON file per qualifying card under tricerules-cards\data\generated\<letter>\;
    build.rs embeds them automatically. Every functional Oracle-text clause must match a typed
    recipe exactly; unsupported or ambiguous cards and faces are skipped without partial output.
    Cards already present in data\ (by id or name) are also skipped.

    Workflow:
        ./scripts/fetch-scryfall-bulk.ps1            # download oracle-cards.jsonl.gz
        ./scripts/gen-cards.ps1 --dry-run            # preview counts + skip reasons
        ./scripts/gen-cards.ps1 --candidate-report build/candidates.json
        ./scripts/gen-cards.ps1                      # write the RON files
        cd tricerules; cargo test                    # registry + conformance validate every card
        ./scripts/gen-card-checklist.ps1 --check     # name gate, then review + commit

    Any extra args pass through to gen-cards (e.g. --limit 50, --out-dir <path>). The default
    --input is oracle-cards.jsonl.gz in the repo root; override with --input <path>.
    Candidate reports are read-only analysis. Add --target-names <path> to restrict one report
    to an exact-name corpus; the file contains one whole-card or face name per nonblank line.
.EXAMPLE
    ./scripts/gen-cards.ps1 --dry-run
#>

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
$manifest = Join-Path $repoRoot "tricerules\Cargo.toml"
$defaultInput = Join-Path $repoRoot "oracle-cards.jsonl.gz"

$extra = @()
if ($args -notcontains "--input") {
    if (-not (Test-Path $defaultInput)) {
        Write-Error "$defaultInput not found - run ./scripts/fetch-scryfall-bulk.ps1 first, or pass --input <path>."
        exit 1
    }
    $extra = @("--input", $defaultInput)
}

cargo run --release --manifest-path $manifest -p tricerules-cards --features gencards --bin gen-cards -- @extra @args
exit $LASTEXITCODE
