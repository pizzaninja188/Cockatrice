<#
.SYNOPSIS
    Run full affected-side Windows verification with retained logs and exact exit codes.
.DESCRIPTION
    Choose Rust, Cpp, or Both after tracing the change's affected components. CardData adds
    read-only canonical card checks and requires Rust. Preview prints the sequence without
    executing it. Focused red/green commands remain on run-quiet-command.ps1.
.EXAMPLE
    ./scripts/verify.ps1 -Side Both -CardData
.EXAMPLE
    ./scripts/verify.ps1 -Side Rust -Preview
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateSet('Rust', 'Cpp', 'Both')]
    [string] $Side,
    [switch] $CardData,
    [switch] $Preview
)

$ErrorActionPreference = 'Stop'
if ($Side -eq 'Cpp' -and $CardData) {
    Write-Error '-CardData requires -Side Rust or Both.' -ErrorAction Continue
    exit 2
}
$repo = Split-Path -Parent $PSScriptRoot
$windowsPowerShell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
$steps = [Collections.Generic.List[object]]::new()

function Add-VerificationStep {
    param([string] $Label, [string] $Executable, [string[]] $Arguments,
        [string] $WorkingDirectory = $repo, [bool] $RequireE2E = $false)
    $steps.Add([pscustomobject][ordered]@{
        Label = $Label
        Executable = $Executable
        Arguments = $Arguments
        WorkingDirectory = $WorkingDirectory
        RequireE2E = $RequireE2E
        Status = 'NotRun'
        ExitCode = $null
        LogPath = $null
    })
}

if ($Side -in @('Rust', 'Both')) {
    $rust = Join-Path $repo 'tricerules'
    Add-VerificationStep 'Rust tests' 'cargo' @('test') $rust
    Add-VerificationStep 'Rust Clippy' 'cargo' @('clippy', '--all-targets', '--', '-D', 'warnings') $rust
    Add-VerificationStep 'Rust formatting' 'cargo' @('fmt', '--check') $rust
}
if ($Side -in @('Cpp', 'Both')) {
    Add-VerificationStep 'Windows Ninja build' $windowsPowerShell @(
        '-NoProfile', '-File', (Join-Path $PSScriptRoot 'build-ninja.ps1')
    )
    Add-VerificationStep 'C++ tests' 'ctest' @(
        '--test-dir', 'build/windows-ninja-all', '--output-on-failure', '--no-tests=error'
    ) $repo $true
}
if ($CardData) {
    Add-VerificationStep 'Card data' $windowsPowerShell @(
        '-NoProfile', '-File', (Join-Path $PSScriptRoot 'update-card-data.ps1'), '-Mode', 'Check'
    )
}
Add-VerificationStep 'Git diff check' 'git' @('diff', '--check')

if ($Preview) {
    foreach ($step in $steps) {
        Write-Output "[$($step.Label)] cwd: $($step.WorkingDirectory)"
        # JSON preserves the argument boundaries; this is display, not shell command text.
        Write-Output (@($step.Executable) + $step.Arguments | ConvertTo-Json -Compress)
        if ($step.RequireE2E) { Write-Output 'Child environment: RULED_E2E_REQUIRE=1' }
    }
    exit 0
}

$runDirectory = Join-Path $repo ('build\verification-logs\verify-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $runDirectory -Force | Out-Null
$summaryPath = Join-Path $runDirectory 'summary.json'
$summary = [ordered]@{
    StartedAt = [DateTime]::UtcNow.ToString('o')
    CompletedAt = $null
    Side = $Side
    CardData = [bool] $CardData
    Status = 'Running'
    ExitCode = $null
    Steps = $steps
}
$code = 0
try {
    foreach ($step in $steps) {
        $previousE2E = $env:RULED_E2E_REQUIRE
        try {
            if ($step.RequireE2E) { $env:RULED_E2E_REQUIRE = '1' }
            $result = & (Join-Path $PSScriptRoot 'run-quiet-command.ps1') `
                -Label $step.Label -Executable $step.Executable -ArgumentList $step.Arguments `
                -WorkingDirectory $step.WorkingDirectory -LogDirectory $runDirectory -AsResultObject
        }
        finally {
            if ($step.RequireE2E) {
                if ($null -eq $previousE2E) { Remove-Item Env:RULED_E2E_REQUIRE -ErrorAction SilentlyContinue }
                else { $env:RULED_E2E_REQUIRE = $previousE2E }
            }
        }
        $step.ExitCode = $result.ExitCode
        $step.LogPath = $result.LogPath
        $step.Status = if ($result.ExitCode -eq 0) { 'Pass' } else { 'Fail' }
        Write-Output $result.Summary
        if ($result.ShowLog) { Get-Content -LiteralPath $result.LogPath }
        $code = $result.ExitCode
        if ($code -ne 0) { break }
    }
}
catch {
    $code = 1
    $step.Status = 'Fail'
    $step.ExitCode = $code
    $step.LogPath = Join-Path $runDirectory 'orchestration-error.log'
    $_ | Out-String | Set-Content -LiteralPath $step.LogPath
    Get-Content -LiteralPath $step.LogPath
}
finally {
    $summary.CompletedAt = [DateTime]::UtcNow.ToString('o')
    $summary.ExitCode = $code
    $summary.Status = if ($code -eq 0) { 'Pass' } else { 'Fail' }
    $summary | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $summaryPath -Encoding UTF8
    foreach ($remaining in $steps | Where-Object { $_.Status -eq 'NotRun' }) {
        Write-Output "NOT RUN [$($remaining.Label)]"
    }
    Write-Output "Verification $($summary.Status) exit $code - summary: $summaryPath"
}
exit $code
