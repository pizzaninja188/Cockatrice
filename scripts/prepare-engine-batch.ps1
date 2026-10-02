<# Focused engine checks and frozen evidence. No card metadata changes or semantic approval. #>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string] $FocusedTests,
    [Parameter(Mandatory)][string] $OutDirectory,
    [string[]] $Path,
    [string[]] $Evidence,
    [ValidateRange(0,192)][int] $Workers = 0
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'focused-test-plan.ps1')
. (Join-Path $PSScriptRoot 'rust-format-checks.ps1')
$savedJobs = $env:CARGO_BUILD_JOBS
$savedThreads = $env:RUST_TEST_THREADS
$summary = $null
try {
    if (-not [IO.Path]::IsPathRooted($FocusedTests)) { $FocusedTests = Join-Path $repo $FocusedTests }
    if (-not [IO.Path]::IsPathRooted($OutDirectory)) { $OutDirectory = Join-Path $repo $OutDirectory }
    if (Test-Path -LiteralPath $OutDirectory) { throw 'Use a new output directory; prior evidence must not be overwritten.' }
    $tests = @(Get-FocusedTestChecks -Manifest $FocusedTests)
    foreach ($item in $Evidence) {
        if (-not (Test-Path -LiteralPath $item -PathType Leaf)) { throw "Missing evidence: $item" }
    }
    $checks = @(Get-RustFormatChecks -Repository $repo) + $tests
    $packages = @($tests.Package | Sort-Object -Unique)
    $lint = @('clippy', '--quiet')
    foreach ($package in $packages) { $lint += @('-p', $package) }
    $checks += @{ Label = 'Engine lint'; Args = $lint + @('--all-targets','--','-D','warnings'); Exact = $false }
    foreach ($features in @($tests.Features | Where-Object { $_ } | Sort-Object -Unique)) {
        $checks += @{ Label = "Engine lint features: $features";
            Args = $lint + @('--all-targets','--features',$features,'--','-D','warnings'); Exact = $false }
    }
    if ($Workers -gt 0) { $env:CARGO_BUILD_JOBS = "$Workers"; $env:RUST_TEST_THREADS = "$Workers" }
    New-Item -ItemType Directory -Path $OutDirectory | Out-Null
    $summary = [ordered]@{ started_at = [datetime]::UtcNow.ToString('o'); completed_at = $null;
        status = 'running'; semantic_approval = $false; final_gate = 'pending'; steps = @() }
    foreach ($check in $checks) {
        $started = [datetime]::UtcNow
        $result = & (Join-Path $PSScriptRoot 'run-quiet-command.ps1') -Label $check.Label `
            -Executable cargo -ArgumentList $check.Args -WorkingDirectory (Join-Path $repo 'tricerules') `
            -LogDirectory $OutDirectory -AsResultObject
        $summary.steps += @{ label = $check.Label; arguments = $check.Args; exit_code = $result.ExitCode;
            started_at = $started.ToString('o'); completed_at = [datetime]::UtcNow.ToString('o'); log = $result.LogPath }
        Write-Host $result.Summary
        if ($result.ShowLog) { Get-Content -LiteralPath $result.LogPath | Out-Host }
        if ($result.ExitCode -ne 0) { $summary.status = 'failed'; exit $result.ExitCode }
        if ($check.Exact) { Assert-ExactTestResult -Label $check.Label -LogPath $result.LogPath }
    }
    $summary.status = 'prepared'
    $summary.completed_at = [datetime]::UtcNow.ToString('o')
    $summaryPath = Join-Path $OutDirectory 'summary.json'
    $summary | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $summaryPath -Encoding UTF8
    if ($Path) {
        $freezeArgs = @((Join-Path $PSScriptRoot 'authoring-batch.py'), '--repo', $repo, 'freeze')
        foreach ($item in $Path) { $freezeArgs += @('--path', $item) }
        foreach ($item in @($Evidence) + @($FocusedTests, $summaryPath) + @($summary.steps.log)) {
            if ($item) { $freezeArgs += @('--evidence', $item) }
        }
        $freezeArgs += @('--out', (Join-Path $OutDirectory 'review'))
        & python @freezeArgs
        if ($LASTEXITCODE -ne 0) { $summary.status = 'freeze_failed'; exit $LASTEXITCODE }
    }
    Write-Output "Prepared engine evidence: $summaryPath; independent review and full final gate remain required."
    exit 0
}
catch {
    if ($summary) { $summary.status = 'failed' }
    Write-Error $_ -ErrorAction Continue
    exit 1
}
finally {
    if ($summary -and $summary.status -ne 'prepared') {
        $summary.completed_at = [datetime]::UtcNow.ToString('o')
        $summary | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $OutDirectory 'summary.json') -Encoding UTF8
    }
    $env:CARGO_BUILD_JOBS = $savedJobs
    $env:RUST_TEST_THREADS = $savedThreads
}
