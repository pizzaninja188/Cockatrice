<# Execute independent scenario rows against unembedded draft RON using the real engine. #>
[CmdletBinding()]
param([Parameter(Mandatory)] [string] $BatchPath)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$savedBatch = $env:TRICERULES_AUTHORING_BATCH
$savedJobs = $env:CARGO_BUILD_JOBS
$savedThreads = $env:RUST_TEST_THREADS
try {
    $env:TRICERULES_AUTHORING_BATCH = (Resolve-Path -LiteralPath $BatchPath).Path
    # Honor ambient worker settings; no task-specific cap is baked into the tool.
    $precheck = & (Join-Path $PSScriptRoot 'run-quiet-command.ps1') -Label 'Draft structural precheck' `
        -WorkingDirectory (Join-Path $repo 'tricerules') -Executable cargo `
        -ArgumentList @('run','--quiet','-p','tricerules-cards','--features','authoring','--bin','card-author','--','validate-batch','--batch',$env:TRICERULES_AUTHORING_BATCH) -AsResultObject
    if ($precheck.ExitCode -ne 0) { Get-Content -LiteralPath $precheck.LogPath | Out-Host; exit $precheck.ExitCode }
    $result = & (Join-Path $PSScriptRoot 'run-quiet-command.ps1') -Label 'Draft semantic rows' `
        -WorkingDirectory (Join-Path $repo 'tricerules') -Executable cargo `
        -ArgumentList @('test','--quiet','-p','tricerules-core','--features','authoring','--test','scenario',
            'authoring_drafts::external_drafts','--','--exact','--ignored') -AsResultObject
    Write-Host $result.Summary
    if ($result.ShowLog) { Get-Content -LiteralPath $result.LogPath | Out-Host }
    if ($result.ExitCode -ne 0) { exit $result.ExitCode }
    if (-not (Select-String -LiteralPath $result.LogPath -Pattern 'test result: ok\. 1 passed; 0 failed; 0 ignored;')) {
        throw "Draft runner executed no test; see $($result.LogPath)"
    }
    exit 0
}
finally {
    $env:TRICERULES_AUTHORING_BATCH = $savedBatch
    $env:CARGO_BUILD_JOBS = $savedJobs
    $env:RUST_TEST_THREADS = $savedThreads
}
