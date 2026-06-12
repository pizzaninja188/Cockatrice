<#
.SYNOPSIS
    Downloads the Scryfall "oracle_cards" bulk data file (one representative printing per card).
.DESCRIPTION
    A single JSON download — no per-card API calls and no rate-limit exposure. Scryfall is the
    authoritative card-data source per CLAUDE.md; a descriptive User-Agent is required by their
    API guidelines.
.EXAMPLE
    ./scripts/fetch-scryfall-bulk.ps1
.EXAMPLE
    ./scripts/fetch-scryfall-bulk.ps1 C:\temp\oracle.json
#>
param(
    [string]$OutFile
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
if (-not $OutFile) { $OutFile = Join-Path $repoRoot "oracle-cards.json" }
$ua = "Cockatrice-tricerules-gencards/1.0 (https://github.com/Cockatrice/Cockatrice)"

Write-Host "Querying Scryfall bulk-data index..."
$index = Invoke-RestMethod -Uri "https://api.scryfall.com/bulk-data" -Headers @{ "User-Agent" = $ua }
$entry = $index.data | Where-Object { $_.type -eq "oracle_cards" } | Select-Object -First 1
if (-not $entry) {
    Write-Error "could not find the oracle_cards download URI in the bulk-data index"
    exit 1
}

Write-Host "Downloading $($entry.download_uri)"
Invoke-WebRequest -Uri $entry.download_uri -Headers @{ "User-Agent" = $ua } -OutFile $OutFile
Write-Host "Wrote $OutFile ($([math]::Round((Get-Item $OutFile).Length / 1MB, 1)) MB)."
