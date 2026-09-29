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
    [ValidateRange(1, 4)] [int] $Workers = 4
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$logs = Join-Path $repo ('build/verification-logs/prepare-card-batch-' + [guid]::NewGuid())
$savedJobs = $env:CARGO_BUILD_JOBS
$savedThreads = $env:RUST_TEST_THREADS
try {
    $env:CARGO_BUILD_JOBS = "$Workers"
    $env:RUST_TEST_THREADS = "$Workers"
    $checks = @(
        @{ Label = 'Canonical card IDs'; Args = @('test', '--quiet', '-p', 'tricerules-cards', '--lib',
            'registry::tests::card_ids_follow_slug_convention', '--', '--exact') },
        @{ Label = 'Card conformance'; Args = @('test', '--quiet', '-p', 'tricerules-core', '--test',
            'conformance', 'registry_execution_matches_reviewed_baseline', '--', '--exact') }
    )
    foreach ($check in $checks) {
        $result = & (Join-Path $PSScriptRoot 'run-quiet-command.ps1') -Label $check.Label `
            -Executable cargo -ArgumentList $check.Args -WorkingDirectory (Join-Path $repo 'tricerules') `
            -LogDirectory $logs -AsResultObject
        Write-Host $result.Summary
        if ($result.ShowLog) { Get-Content -LiteralPath $result.LogPath | Out-Host }
        if ($result.ExitCode -ne 0) { exit $result.ExitCode }
        # An exact filter that silently matches nothing must not authorize refresh.
        if (-not (Select-String -LiteralPath $result.LogPath -Pattern 'test result: ok\. 1 passed; 0 failed; 0 ignored;')) {
            throw "Expected one executed, non-ignored test for $($check.Label); see $($result.LogPath)"
        }
    }
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
