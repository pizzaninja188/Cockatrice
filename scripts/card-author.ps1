<# Offline analogue search, clone preparation and research queue. See CARD-AUTHORING.md. #>
# Basic script arguments preserve native --flags under Windows PowerShell -File.
[string[]] $CommandArgs = $args
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$savedJobs = $env:CARGO_BUILD_JOBS
$savedThreads = $env:RUST_TEST_THREADS
try {
    # Honor ambient worker settings, including explicit campaign caps.
    $result = & (Join-Path $PSScriptRoot 'run-quiet-command.ps1') -Label 'Offline card author' `
        -WorkingDirectory $repo -Executable cargo -ArgumentList (@('run','--quiet','--manifest-path',
        (Join-Path $repo 'tricerules/Cargo.toml'),'-p','tricerules-cards','--features','authoring','--bin','card-author','--') + $CommandArgs) -AsResultObject
    Get-Content -LiteralPath $result.LogPath | Out-Host
    Write-Host $result.Summary
    exit $result.ExitCode
}
finally { $env:CARGO_BUILD_JOBS = $savedJobs; $env:RUST_TEST_THREADS = $savedThreads }
