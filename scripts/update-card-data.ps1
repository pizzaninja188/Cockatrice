<#
.SYNOPSIS
    Check canonical card data without editing it, or explicitly refresh existing generated data.
.DESCRIPTION
    Check writes only logs and a temporary checklist under build. Refresh regenerates existing
    provenance-owned RON and fingerprints, validates the new checklist before replacing CARDS.md,
    then checks the result. Neither mode downloads sources or includes newly qualifying cards.
    Relative source paths are resolved from the repository root, independent of the caller's cwd.
#>
[CmdletBinding()]
param(
    [ValidateSet('Check', 'Refresh')]
    [string] $Mode = 'Check',
    [string] $OracleBulk,
    [string] $CardsXml
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$windowsPowerShell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
$runner = Join-Path $PSScriptRoot 'run-quiet-command.ps1'

function Resolve-CardSource {
    param([string] $Path)
    if (-not [IO.Path]::IsPathRooted($Path)) { $Path = Join-Path $repo $Path }
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Missing card-data source: $Path" }
    return (Resolve-Path -LiteralPath $Path).Path
}

function Invoke-CardTool {
    param([string] $Label, [string] $Script, [string[]] $Arguments)
    $result = & $runner -Label $Label -Executable $windowsPowerShell `
        -ArgumentList (@('-NoProfile', '-File', (Join-Path $PSScriptRoot $Script)) + $Arguments) `
        -WorkingDirectory $repo -LogDirectory $runDirectory -AsResultObject
    Write-Host $result.Summary
    if ($result.ShowLog) { Get-Content -LiteralPath $result.LogPath | ForEach-Object { Write-Host $_ } }
    return $result.ExitCode
}

function Test-CanonicalCardData {
    $code = Invoke-CardTool 'Generated card check' 'gen-cards.ps1' @('--input', $OracleBulk, '--check')
    if ($code -ne 0) { return $code }
    $code = Invoke-CardTool 'Checklist name validation' 'gen-card-checklist.ps1' @(
        '--cards-xml', $CardsXml, '--out', $candidate, '--check'
    )
    if ($code -ne 0) { return $code }
    if (-not (Test-Path -LiteralPath $checklist -PathType Leaf)) {
        Write-Host "FAIL missing checklist: $checklist"
        return 1
    }
    # Git's Windows checkout can change line endings without changing generated content.
    $actual = [IO.File]::ReadAllText($checklist).Replace("`r`n", "`n")
    $expected = [IO.File]::ReadAllText($candidate).Replace("`r`n", "`n")
    if ($actual -cne $expected) {
        Write-Host "FAIL checklist content drift: $checklist"
        Write-Host "Expected checklist retained at: $candidate"
        return 1
    }
    Write-Host 'PASS canonical card data (tracked files unchanged by Check)'
    return 0
}

try {
    if (-not $OracleBulk) { $OracleBulk = 'oracle-cards.jsonl.gz' }
    if (-not $CardsXml) {
        if (-not $env:LOCALAPPDATA) { throw 'LOCALAPPDATA is unavailable; pass -CardsXml.' }
        $CardsXml = Join-Path $env:LOCALAPPDATA 'Cockatrice\Cockatrice\cards.xml'
    }
    # Check all source presence before Refresh can write anything. gen-cards verifies the SHA.
    $OracleBulk = Resolve-CardSource $OracleBulk
    $CardsXml = Resolve-CardSource $CardsXml
    $null = Resolve-CardSource "$OracleBulk.meta.json"
    $runDirectory = Join-Path $repo ('build\verification-logs\card-data-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $runDirectory -Force | Out-Null
    $candidate = Join-Path $runDirectory 'CARDS.expected.md'
    $checklist = Join-Path $repo 'tricerules\CARDS.md'

    if ($Mode -eq 'Refresh') {
        $code = Invoke-CardTool 'Refresh existing generated cards' 'gen-cards.ps1' @('--input', $OracleBulk)
        if ($code -ne 0) { exit $code }
        $code = Invoke-CardTool 'Validate refreshed checklist' 'gen-card-checklist.ps1' @(
            '--cards-xml', $CardsXml, '--out', $candidate, '--check'
        )
        if ($code -ne 0) { exit $code }
        Copy-Item -LiteralPath $candidate -Destination $checklist -Force
        Write-Host 'Refreshed existing generated data. Review the generated RON, fingerprint, and CARDS.md diff before staging.'
    }
    exit (Test-CanonicalCardData)
}
catch {
    Write-Error $_ -ErrorAction Continue
    exit 1
}
