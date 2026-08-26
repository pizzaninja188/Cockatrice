<#
.SYNOPSIS
    Downloads Scryfall gzipped JSONL bulk data with a verified provenance sidecar.
.DESCRIPTION
    A single bulk download - no per-card API calls and no rate-limit exposure. Scryfall is the
    authoritative card-data source per AGENTS.md; a descriptive User-Agent is required by their
    API guidelines.
.EXAMPLE
    ./scripts/fetch-scryfall-bulk.ps1
.EXAMPLE
    ./scripts/fetch-scryfall-bulk.ps1 C:\temp\oracle-cards.jsonl.gz
#>
param(
    [string]$OutFile,
    [switch]$IncludeOracleTags,
    [string]$TagsOutFile
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
if (-not $OutFile) { $OutFile = Join-Path $repoRoot "oracle-cards.jsonl.gz" }
if (-not $TagsOutFile) { $TagsOutFile = Join-Path $repoRoot "oracle-tags.jsonl.gz" }
$ua = "Cockatrice-tricerules-gencards/1.0 (https://github.com/Cockatrice/Cockatrice)"
$headers = @{ "User-Agent" = $ua; "Accept" = "application/json" }

Write-Host "Querying Scryfall bulk-data index..."
$index = Invoke-RestMethod -Uri "https://api.scryfall.com/bulk-data" -Headers $headers
function Save-BulkFile([string]$Type, [string]$Path) {
    $entry = $index.data | Where-Object { $_.type -eq $Type } | Select-Object -First 1
    if (-not $entry -or -not $entry.jsonl_download_uri) {
        throw "could not find the $Type jsonl_download_uri in the bulk-data index"
    }

    Write-Host "Downloading $($entry.jsonl_download_uri)"
    Invoke-WebRequest -Uri $entry.jsonl_download_uri -Headers $headers -OutFile $Path
    $sha256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    $metadata = [ordered]@{
        type = $entry.type
        id = $entry.id
        updated_at = $entry.updated_at
        jsonl_download_uri = $entry.jsonl_download_uri
        sha256 = $sha256
    } | ConvertTo-Json
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText("$Path.meta.json", $metadata, $utf8NoBom)
    Write-Host "Wrote $Path ($([math]::Round((Get-Item -LiteralPath $Path).Length / 1MB, 1)) MB)"
    Write-Host "Wrote $Path.meta.json (sha256:$sha256)"
}

Save-BulkFile "oracle_cards" $OutFile
if ($IncludeOracleTags) {
    Save-BulkFile "oracle_tags" $TagsOutFile
}
