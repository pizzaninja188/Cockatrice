<#
.SYNOPSIS
    Validate checked-in direct-RON maps and their non-ignored Cargo test references.
.DESCRIPTION
    This checks structure and executable test identity, not semantic equivalence or test success.
    The full Rust gate must also pass on the same content. Only build artifacts are written.
    Map filenames must equal the canonical handwritten RON filename stem. References use either
    "scenario module::test" (tricerules-core) or "integration_target::test" (tricerules-cards).
#>
[CmdletBinding()]
param([string] $OracleBulk)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if ($OracleBulk -and -not [IO.Path]::IsPathRooted($OracleBulk)) { $OracleBulk = Join-Path $repo $OracleBulk }
Import-Module (Join-Path $PSScriptRoot 'card-evidence.psm1') -Force
$maps = @(Get-ChildItem -LiteralPath (Join-Path $repo 'tricerules/tricerules-cards/authoring/review-maps') -Filter '*.json')
if ($maps.Count -eq 0) { throw 'No direct-RON review maps found' }
$logs = Join-Path $repo ('build/verification-logs/card-evidence-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $logs -Force | Out-Null
function Invoke-EvidenceTool {
    param([string] $Label, [string] $Executable, [string[]] $Arguments)
    $result = & (Join-Path $PSScriptRoot 'run-quiet-command.ps1') -Label $Label `
        -Executable $Executable -ArgumentList $Arguments -WorkingDirectory (Join-Path $repo 'tricerules') `
        -LogDirectory $logs -AsResultObject
    Write-Host $result.Summary
    if ($result.ShowLog) { Get-Content -LiteralPath $result.LogPath | Out-Host }
    if ($result.ExitCode -ne 0) { throw "$Label failed with exit $($result.ExitCode)" }
    return @(Get-Content -LiteralPath $result.LogPath)
}
try {
    $groups = @{}
    $arguments = @('-NoProfile', '-File', (Join-Path $repo 'scripts/gen-cards.ps1'),
        '--review-existing', '--review-draft', (Join-Path $repo 'tricerules/tricerules-cards/data'),
        '--review-map', (Join-Path $repo 'tricerules/tricerules-cards/authoring/review-maps'),
        '--review-out', (Join-Path $logs 'packets'))
    if ($OracleBulk) { $arguments += @('--input', $OracleBulk) }
    $null = Invoke-EvidenceTool 'Review map batch' `
        (Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/powershell.exe') $arguments
    foreach ($map in $maps) {
        $definition = Get-Content -LiteralPath $map.FullName -Raw | ConvertFrom-Json
        if (@($definition.semantic_fixtures).Count -eq 0) { throw "No semantic fixtures in $($map.Name)" }
        foreach ($fixture in $definition.semantic_fixtures) {
            $reference = [string] $fixture.test
            if ($reference -cmatch '^scenario ([A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)*)$') {
                $key = 'tricerules-core/scenario'
            }
            elseif ($reference -cmatch '^([A-Za-z0-9_]+)::([A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)*)$') {
                $key = "tricerules-cards/$($Matches[1])"
            }
            else { throw "Unsupported semantic test reference in $($map.Name): $reference" }
            if (-not $groups.ContainsKey($key)) { $groups[$key] = @() }
            $groups[$key] += $reference
        }
    }
    foreach ($key in ($groups.Keys | Sort-Object)) {
        $package, $target = $key.Split('/')
        $prefix = if ($package -eq 'tricerules-core') { 'scenario ' } else { "$target`::" }
        $arguments = @('test', '--quiet', '-p', $package, '--test', $target, '--', '--list')
        $listed = @(Invoke-EvidenceTool "List $target tests" 'cargo' $arguments)
        $ignoredList = @(Invoke-EvidenceTool "List $target ignored tests" 'cargo' ($arguments + '--ignored'))
        $available = @($listed | Where-Object { $_ -cmatch '^(.+): test$' } | ForEach-Object { $prefix + ($_ -creplace ': test$', '') })
        $ignored = @($ignoredList | Where-Object { $_ -cmatch '^(.+): test$' } | ForEach-Object { $prefix + ($_ -creplace ': test$', '') })
        foreach ($reference in $groups[$key]) { Assert-CardEvidenceReference $reference $available $ignored }
    }
    Write-Output "PASS $($maps.Count) direct-RON review maps and their executable test references"
    exit 0
}
catch {
    Write-Error $_ -ErrorAction Continue
    exit 1
}
