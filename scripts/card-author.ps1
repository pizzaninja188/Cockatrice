<# Offline analogue search, clone preparation and research queue. See CARD-AUTHORING.md. #>
[CmdletBinding()]
param([Parameter(ValueFromRemainingArguments)] [string[]] $CommandArgs)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$savedJobs = $env:CARGO_BUILD_JOBS
$savedThreads = $env:RUST_TEST_THREADS
try {
    $env:CARGO_BUILD_JOBS = '4'
    $env:RUST_TEST_THREADS = '4'
    $result = & (Join-Path $PSScriptRoot 'run-quiet-command.ps1') -Label 'Offline card author' `
        -WorkingDirectory $repo -Executable cargo -ArgumentList (@('run','--quiet','--manifest-path',
        (Join-Path $repo 'tricerules/Cargo.toml'),'-p','tricerules-cards','--features','authoring','--bin','card-author','--') + $CommandArgs) -AsResultObject
    Get-Content -LiteralPath $result.LogPath | Out-Host
    Write-Host $result.Summary
    exit $result.ExitCode
}
finally { $env:CARGO_BUILD_JOBS = $savedJobs; $env:RUST_TEST_THREADS = $savedThreads }
