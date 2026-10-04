<#
.SYNOPSIS
    Check card identity and generic execution before refreshing handwritten-card metadata.
.DESCRIPTION
    Run after focused semantic green, before independent review. This is preparation, not
    semantic approval or final verification. Sources remain local; no cards are admitted.
#>
[CmdletBinding()]
param(
    [string] $OracleBulk,
    [string] $CardsXml,
    [string] $FocusedTests,
    [string[]] $ReviewMapPath,
    [ValidateRange(0, 192)] [int] $Workers = 0
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'focused-test-plan.ps1')
. (Join-Path $PSScriptRoot 'rust-format-checks.ps1')
$logs = Join-Path $repo ('build/verification-logs/prepare-card-batch-' + [guid]::NewGuid())
$savedJobs = $env:CARGO_BUILD_JOBS
$savedThreads = $env:RUST_TEST_THREADS
try {
    if ($Workers -gt 0) {
        $env:CARGO_BUILD_JOBS = "$Workers"
        $env:RUST_TEST_THREADS = "$Workers"
    }
    $checks = @(Get-RustFormatChecks -Repository $repo) + @(
        @{ Label = 'Canonical card IDs'; Args = @('test', '--quiet', '-p', 'tricerules-cards', '--lib',
            'registry::tests::card_ids_follow_slug_convention', '--', '--exact'); Exact = $true },
        @{ Label = 'Card conformance'; Args = @('test', '--quiet', '-p', 'tricerules-core', '--test',
            'conformance', 'registry_execution_matches_reviewed_baseline', '--', '--exact'); Exact = $true }
    )
    if ($FocusedTests) {
        if (-not [IO.Path]::IsPathRooted($FocusedTests)) { $FocusedTests = Join-Path $repo $FocusedTests }
        $checks += @(Get-FocusedTestChecks -Manifest $FocusedTests)
    }
    $checks += @{ Label = 'Authoring lint'; Args = @('clippy', '--quiet', '-p', 'tricerules-cards', '-p',
        'tricerules-core', '--all-targets', '--features', 'tricerules-cards/authoring', '--', '-D', 'warnings'); Exact = $false }
    foreach ($check in $checks) {
        $executable = if ($check.Executable) { $check.Executable } else { 'cargo' }
        $result = & (Join-Path $PSScriptRoot 'run-quiet-command.ps1') -Label $check.Label `
            -Executable $executable -ArgumentList $check.Args -WorkingDirectory (Join-Path $repo 'tricerules') `
            -LogDirectory $logs -AsResultObject
        Write-Host $result.Summary
        if ($result.ShowLog) { Get-Content -LiteralPath $result.LogPath | Out-Host }
        if ($result.ExitCode -ne 0) { exit $result.ExitCode }
        # An exact filter that silently matches nothing must not authorize refresh.
        if ($check.Exact) { Assert-ExactTestResult -Label $check.Label -LogPath $result.LogPath }
    }
    # Full evidence structure and executable references, without claiming semantic approval.
    $evidenceArgs = @('-NoProfile', '-File', (Join-Path $repo 'scripts/check-card-evidence.ps1'), '-Preparation')
    if ($OracleBulk) {
        $evidenceBulk = $OracleBulk
        if (-not [IO.Path]::IsPathRooted($evidenceBulk)) { $evidenceBulk = Join-Path $repo $evidenceBulk }
        $evidenceArgs += @('-OracleBulk', $evidenceBulk)
    }
    if ($ReviewMapPath) {
        $mapListPath = Join-Path $logs 'selected-review-maps.json'
        [IO.File]::WriteAllText($mapListPath, (ConvertTo-Json -InputObject @($ReviewMapPath) -Compress))
        $evidenceArgs += @('-MapListFile', $mapListPath)
    }
    & (Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/powershell.exe') @evidenceArgs
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    $arguments = @('-Mode', 'Refresh', '-MetadataOnly')
    if ($OracleBulk) { $arguments += @('-OracleBulk', $OracleBulk) }
    if ($CardsXml) { $arguments += @('-CardsXml', $CardsXml) }
    & (Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/powershell.exe') -NoProfile `
        -File (Join-Path $PSScriptRoot 'update-card-data.ps1') @arguments
    exit $LASTEXITCODE
}
catch {
    Write-Error $_ -ErrorAction Continue
    exit 1
}
finally {
    $env:CARGO_BUILD_JOBS = $savedJobs
    $env:RUST_TEST_THREADS = $savedThreads
}
