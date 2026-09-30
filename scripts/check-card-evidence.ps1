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
param([string] $OracleBulk, [switch] $Preparation, [string] $MapListJson, [string] $MapListFile)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if ($OracleBulk -and -not [IO.Path]::IsPathRooted($OracleBulk)) { $OracleBulk = Join-Path $repo $OracleBulk }
Import-Module (Join-Path $PSScriptRoot 'card-evidence.psm1') -Force
$maps = @(Get-ChildItem -LiteralPath (Join-Path $repo 'tricerules/tricerules-cards/authoring/review-maps') -Filter '*.json')
if ($MapListJson -and $MapListFile) { throw 'Specify only one map selection input.' }
if ($MapListJson -or $MapListFile) {
    if (-not $Preparation) { throw 'Map selection is preparation-only; final evidence checks the full corpus.' }
    $mapSelection = if ($MapListFile) {
        Get-Content -LiteralPath $MapListFile -Raw
    } else {
        $MapListJson
    }
    $parsedMaps = ConvertFrom-Json $mapSelection
    $requested = @($parsedMaps)
    if (-not $requested.Count) { throw 'Empty map selection.' }
    $maps = @($requested | ForEach-Object {
        $path = [string]$_
        if (-not [IO.Path]::IsPathRooted($path)) { $path = Join-Path $repo $path }
        $item = Get-Item -LiteralPath $path
        $mapDirectory = [IO.Path]::GetFullPath((Join-Path $repo 'tricerules/tricerules-cards/authoring/review-maps'))
        if ($item.Extension -ne '.json' -or $item.DirectoryName -ne $mapDirectory) { throw 'Map must be a checked-in review-map path.' }
        $item
    })
}
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
    if ($Preparation) {
        $arguments += '--review-preparation'
        if ($MapListJson -or $MapListFile) {
            $selected = Join-Path $logs 'selected-maps'
            New-Item -ItemType Directory -Path $selected | Out-Null
            foreach ($map in $maps) {
                $destination = Join-Path $selected $map.Name
                if (Test-Path -LiteralPath $destination) { throw "Duplicate selected map: $($map.Name)" }
                Copy-Item -LiteralPath $map.FullName -Destination $destination
            }
            $mapIndex = [Array]::IndexOf($arguments, '--review-map')
            $arguments[$mapIndex + 1] = $selected
        }
    }
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
    # Ask Cargo for fresh artifacts once per package, not twice per test target.
    # Never glob target/debug: that can select stale binaries from an older build.
    $executables = @{}
    foreach ($package in @('tricerules-cards', 'tricerules-core')) {
        $targets = @($groups.Keys | Where-Object { $_.StartsWith("$package/") } |
            ForEach-Object { $_.Split('/')[1] } | Sort-Object -Unique)
        if ($targets.Count -eq 0) { continue }
        $arguments = @('test', '--quiet', '-p', $package, '--no-run', '--message-format=json')
        foreach ($target in $targets) { $arguments += @('--test', $target) }
        $artifacts = @(Invoke-EvidenceTool "Build $package evidence tests" 'cargo' $arguments)
        $resolved = Get-CardEvidenceExecutables $artifacts $targets
        foreach ($target in $targets) { $executables["$package/$target"] = $resolved[$target] }
    }
    foreach ($key in ($groups.Keys | Sort-Object)) {
        $package, $target = $key.Split('/')
        $prefix = if ($package -eq 'tricerules-core') { 'scenario ' } else { "$target`::" }
        $listed = @(Invoke-EvidenceTool "List $target tests" $executables[$key] @('--list'))
        $ignoredList = @(Invoke-EvidenceTool "List $target ignored tests" $executables[$key] @('--list', '--ignored'))
        $available = @($listed | Where-Object { $_ -cmatch '^(.+): test$' } | ForEach-Object { $prefix + ($_ -creplace ': test$', '') })
        $ignored = @($ignoredList | Where-Object { $_ -cmatch '^(.+): test$' } | ForEach-Object { $prefix + ($_ -creplace ': test$', '') })
        foreach ($reference in $groups[$key]) { Assert-CardEvidenceReference $reference $available $ignored }
    }
    Write-Output "PASS $($maps.Count) direct-RON review maps and their executable test references (preparation=$Preparation; semantic approval is separate)"
    exit 0
}
catch {
    Write-Error $_ -ErrorAction Continue
    exit 1
}
