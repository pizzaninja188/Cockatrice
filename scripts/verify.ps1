<#
.SYNOPSIS
    Run full affected-side Windows or Linux verification with retained logs and exact exit codes.
.DESCRIPTION
    Choose Rust, Cpp, or Both after tracing the change's affected components. CardData adds
    read-only canonical card checks and requires Rust. RecordRoutineBaseline captures stable
    inputs for a successful full Rust/CardData run. RoutineRonBaseline opts into a narrower
    gate, subject to that baseline and strict eligibility. Preview prints the full sequence
    without executing it. Focused red/green commands remain on run-quiet-command.ps1.
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
    [switch] $Preview,
    [switch] $RecordRoutineBaseline,
    [string] $RoutineRonBaseline,
    [string] $OracleBulk,
    [string] $CardsXml,
    [ValidateRange(1, 1000000)]
    [int] $MinimumFreeGiB
)

$ErrorActionPreference = 'Stop'
if ($Side -eq 'Cpp' -and $CardData) {
    Write-Error '-CardData requires -Side Rust or Both.' -ErrorAction Continue
    exit 2
}
if (($RecordRoutineBaseline -or $RoutineRonBaseline) -and (-not $CardData -or $Side -eq 'Cpp')) {
    Write-Error 'Routine RON baselines require Rust and -CardData.' -ErrorAction Continue
    exit 2
}
if ($RoutineRonBaseline -and ($Side -ne 'Rust' -or $RecordRoutineBaseline)) {
    Write-Error 'Routine RON verification requires -Side Rust and cannot record a full baseline.' -ErrorAction Continue
    exit 2
}
if ($Preview -and ($RoutineRonBaseline -or $RecordRoutineBaseline)) {
    Write-Error 'Baseline recording and routine RON selection require a real run; Preview executes no commands.' -ErrorAction Continue
    exit 2
}
$repo = Split-Path -Parent $PSScriptRoot
$windowsPowerShell = (Get-Process -Id $PID).Path
$isWindowsHost = [IO.Path]::DirectorySeparatorChar -eq '\'
$buildPlatform = if ($isWindowsHost) { 'Windows' } else { 'Linux' }
$buildScript = if ($isWindowsHost) { 'build-ninja.ps1' } else { 'build-linux.ps1' }
$buildDirectory = if ($isWindowsHost) { 'build/windows-ninja-all' } else { 'build/linux-ninja-all' }
$environmentArguments = @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'check-verification-environment.ps1'))
if ($PSBoundParameters.ContainsKey('MinimumFreeGiB')) { $environmentArguments += @('-MinimumFreeGiB', [string] $MinimumFreeGiB) }

function Add-VerificationStep {
    param([string] $Label, [string] $Executable, [string[]] $Arguments,
        [string] $WorkingDirectory = $repo, [bool] $RequireE2E = $false, [bool] $Exact = $false)
    $steps.Add([pscustomobject][ordered]@{
        Label = $Label
        Executable = $Executable
        Arguments = $Arguments
        WorkingDirectory = $WorkingDirectory
        RequireE2E = $RequireE2E
        Exact = $Exact
        Status = 'NotRun'
        ExitCode = $null
        LogPath = $null
    })
}

function Get-FullVerificationSteps {
    param([string] $SelectedSide, [bool] $CheckCardData)
    $steps = [Collections.Generic.List[object]]::new()
    $preflight = $environmentArguments
    if ($SelectedSide -eq 'Cpp') { $preflight += '-SkipRustCache' }
    Add-VerificationStep 'Verification environment' $windowsPowerShell $preflight
    if ($CheckCardData) {
        $cardArguments = @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'update-card-data.ps1'), '-Mode', 'Check')
        if ($OracleBulk) { $cardArguments += @('-OracleBulk', $OracleBulk) }
        if ($CardsXml) { $cardArguments += @('-CardsXml', $CardsXml) }
        Add-VerificationStep 'Card data' $windowsPowerShell $cardArguments
    }
    if ($SelectedSide -in @('Rust', 'Both')) {
        $rust = Join-Path $repo 'tricerules'
        Add-VerificationStep 'Card data dependency boundary' $windowsPowerShell @(
            '-NoProfile', '-File', (Join-Path $repo 'tests/scripts/card_data_boundary_test.ps1')
        )
        Add-VerificationStep 'Rust tests' 'cargo' @('test') $rust
        Add-VerificationStep 'Rust Clippy' 'cargo' @('clippy', '--all-targets', '--', '-D', 'warnings') $rust
        . (Join-Path $PSScriptRoot 'rust-format-checks.ps1')
        foreach ($format in @(Get-RustFormatChecks -Repository $repo)) {
            Add-VerificationStep $format.Label $format.Executable $format.Args $rust
        }
    }
    if ($SelectedSide -in @('Cpp', 'Both')) {
        Add-VerificationStep "$buildPlatform Ninja build" $windowsPowerShell @(
            '-NoProfile', '-File', (Join-Path $PSScriptRoot $buildScript)
        )
        Add-VerificationStep 'C++ tests' 'ctest' @(
            '--test-dir', $buildDirectory, '--output-on-failure', '--no-tests=error'
        ) $repo $true
    }
    Add-VerificationStep 'Git diff check' 'git' @('diff', '--check')
    return $steps.ToArray()
}
$steps = [Collections.Generic.List[object]]::new()
foreach ($fullStep in @(Get-FullVerificationSteps -SelectedSide $Side -CheckCardData ([bool] $CardData))) { $steps.Add($fullStep) }

if ($Preview) {
    foreach ($step in $steps) {
        Write-Output "[$($step.Label)] cwd: $($step.WorkingDirectory)"
        # JSON preserves the argument boundaries; this is display, not shell command text.
        Write-Output (@($step.Executable) + $step.Arguments | ConvertTo-Json -Compress)
        if ($step.RequireE2E) { Write-Output 'Child environment: RULED_E2E_REQUIRE=1' }
        if ($step.RequireE2E -and -not $isWindowsHost) { Write-Output 'Child environment: QT_QPA_PLATFORM=offscreen' }
    }
    exit 0
}

$leaseDirectory = Join-Path $repo 'build'
$originalCacheBudget = $env:COCKATRICE_RUST_CACHE_BUDGET_GIB
$cacheBudget = $originalCacheBudget
if ($Side -ne 'Cpp' -and -not $cacheBudget) {
    $budgetFile = Join-Path $leaseDirectory 'rust-cache-budget-gib'
    if (Test-Path -LiteralPath $budgetFile -PathType Leaf) {
        $cacheBudget = (Get-Content -LiteralPath $budgetFile -Raw).Trim()
    }
}
New-Item -ItemType Directory -Path $leaseDirectory -Force | Out-Null
$leasePath = Join-Path $leaseDirectory '.verification.lock'
# Normal gates can share a lease. A managed gate holds it exclusively through the
# build and test commands so another gate cannot recycle the cache between steps.
try {
    if (-not [IO.File]::Exists($leasePath)) {
        try { [IO.File]::Open($leasePath, 'CreateNew', 'Write', 'ReadWrite').Dispose() }
        catch [IO.IOException] { if (-not [IO.File]::Exists($leasePath)) { throw } }
    }
    $lease = if ($Side -ne 'Cpp' -and $cacheBudget) {
        [IO.File]::Open($leasePath, 'Open', 'ReadWrite', 'None')
    } else {
        [IO.File]::Open($leasePath, 'Open', 'Read', 'Read')
    }
}
catch [IO.IOException] {
    Write-Error "Could not acquire the checkout's verification lease. Retry after managed verification finishes. $_" -ErrorAction Continue
    exit 1
}

$runDirectory = Join-Path $repo ('build\verification-logs\verify-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $runDirectory -Force | Out-Null
$summaryPath = Join-Path $runDirectory 'summary.json'
$summary = [ordered]@{
    StartedAt = [DateTime]::UtcNow.ToString('o')
    CompletedAt = $null
    Side = $Side
    CardData = [bool] $CardData
    RecordRoutineBaseline = [bool] $RecordRoutineBaseline
    RoutineRon = [bool] $RoutineRonBaseline
    RoutineBaseline = $null
    RoutineCards = @()
    RoutineChangedPaths = @()
    Status = 'Running'
    ExitCode = $null
    Steps = $steps
}
$code = 0
$currentStep = $null
try {
    if ($RecordRoutineBaseline -or $RoutineRonBaseline) {
        . (Join-Path $PSScriptRoot 'routine-ron-checks.ps1')
        if ($RoutineRonBaseline) {
            if (-not [IO.Path]::IsPathRooted($RoutineRonBaseline)) { $RoutineRonBaseline = Join-Path $repo $RoutineRonBaseline }
            $candidateSummary = Get-Content -LiteralPath $RoutineRonBaseline -Raw | ConvertFrom-Json
            if ($candidateSummary.Side -cnotin @('Rust', 'Both')) { throw 'Invalid full-gate baseline side.' }
            $expected = @(Get-FullVerificationSteps -SelectedSide $candidateSummary.Side -CheckCardData $true)
            $baseline = Read-RoutineRonBaseline -SummaryPath $RoutineRonBaseline -ExpectedSteps $expected
        }
        $startSnapshot = Get-VerificationInputSnapshot -Repository $repo
        $startJson = ConvertTo-VerificationSnapshotJson $startSnapshot
        if ($RoutineRonBaseline) {
            $plan = Get-RoutineRonPlan -Repository $repo -Baseline $baseline -Current $startSnapshot
            $summary.RoutineCards = $plan.Cards
            $summary.RoutineChangedPaths = $plan.ChangedPaths
            $summary.BaselineSummary = [IO.Path]::GetFullPath($RoutineRonBaseline)
            $fullSteps = @($steps.ToArray())
            $steps = [Collections.Generic.List[object]]::new()
            foreach ($fullStep in $fullSteps) {
                if ($fullStep.Label -ne 'Rust tests') { $steps.Add($fullStep); continue }
                $rust = Join-Path $repo 'tricerules'
                Add-VerificationStep 'Card registry tests' 'cargo' @('test', '-p', 'tricerules-cards') $rust
                Add-VerificationStep 'Card conformance tests' 'cargo' @('test', '-p', 'tricerules-core', '--test', 'conformance') $rust
                foreach ($test in $plan.Tests) {
                    Add-VerificationStep "Card semantic test: $($test.Target)::$($test.Name)" 'cargo' @(
                        'test', '--quiet', '-p', $test.Package, '--test', $test.Target, $test.Name, '--', '--exact'
                    ) $rust $false $true
                }
            }
            $summary.Steps = $steps
        }
    }
    foreach ($step in $steps) {
        $currentStep = $step
        $previousE2E = $env:RULED_E2E_REQUIRE
        $previousQpa = $env:QT_QPA_PLATFORM
        try {
            if ($step.RequireE2E) { $env:RULED_E2E_REQUIRE = '1' }
            if ($step.RequireE2E -and -not $isWindowsHost) { $env:QT_QPA_PLATFORM = 'offscreen' }
            if ($step.Label -eq 'Verification environment' -and $cacheBudget) {
                $env:COCKATRICE_RUST_CACHE_BUDGET_GIB = $cacheBudget
            }
            $result = & (Join-Path $PSScriptRoot 'run-quiet-command.ps1') `
                -Label $step.Label -Executable $step.Executable -ArgumentList $step.Arguments `
                -WorkingDirectory $step.WorkingDirectory -LogDirectory $runDirectory -AsResultObject
        }
        finally {
            if ($step.Label -eq 'Verification environment') {
                $env:COCKATRICE_RUST_CACHE_BUDGET_GIB = $originalCacheBudget
            }
            if ($step.RequireE2E) {
                if ($null -eq $previousE2E) { Remove-Item Env:RULED_E2E_REQUIRE -ErrorAction SilentlyContinue }
                else { $env:RULED_E2E_REQUIRE = $previousE2E }
                if (-not $isWindowsHost) {
                    if ($null -eq $previousQpa) { Remove-Item Env:QT_QPA_PLATFORM -ErrorAction SilentlyContinue }
                    else { $env:QT_QPA_PLATFORM = $previousQpa }
                }
            }
        }
        if ($step.Exact -and $result.ExitCode -eq 0 -and -not (Select-String -LiteralPath $result.LogPath -Quiet -Pattern 'test result: ok\. 1 passed; 0 failed; 0 ignored;')) {
            Add-Content -LiteralPath $result.LogPath -Value 'Expected exactly one executed, non-ignored semantic test.'
            $result.ExitCode = 1
            $result.ShowLog = $true
            $result.Summary = "FAIL [$($step.Label)] exit 1 - log: $($result.LogPath)"
        }
        $step.ExitCode = $result.ExitCode
        $step.LogPath = $result.LogPath
        $step.Status = if ($result.ExitCode -eq 0) { 'Pass' } else { 'Fail' }
        Write-Output $result.Summary
        if ($result.ShowLog) { Get-Content -LiteralPath $result.LogPath }
        $code = $result.ExitCode
        $currentStep = $null
        if ($code -ne 0) { break }
    }
    if ($code -eq 0 -and ($RecordRoutineBaseline -or $RoutineRonBaseline)) {
        $endJson = ConvertTo-VerificationSnapshotJson (Get-VerificationInputSnapshot -Repository $repo)
        if ($startJson -cne $endJson) { throw 'Verification inputs or environment changed during the run.' }
        if ($RecordRoutineBaseline) {
            $snapshotPath = Join-Path $runDirectory 'routine-inputs.json'
            [IO.File]::WriteAllText($snapshotPath, $startJson, [Text.UTF8Encoding]::new($false))
            $hash = (Get-FileHash -LiteralPath $snapshotPath -Algorithm SHA256).Hash
            $summary.RoutineBaseline = [ordered]@{ Version = 1; SnapshotFile = 'routine-inputs.json'; StartSha256 = $hash; EndSha256 = $hash }
        }
    }
}
catch {
    $code = 1
    $errorPath = Join-Path $runDirectory 'orchestration-error.log'
    $_ | Out-String | Set-Content -LiteralPath $errorPath
    if ($null -ne $currentStep) {
        $currentStep.Status = 'Fail'
        $currentStep.ExitCode = $code
        $currentStep.LogPath = $errorPath
    }
    $summary.OrchestrationError = $errorPath
    Get-Content -LiteralPath $errorPath
}
finally {
    $lease.Dispose()
    $summary.CompletedAt = [DateTime]::UtcNow.ToString('o')
    $summary.ExitCode = $code
    $summary.Status = if ($code -eq 0) { 'Pass' } else { 'Fail' }
    $summary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $summaryPath -Encoding UTF8
    foreach ($remaining in $steps | Where-Object { $_.Status -eq 'NotRun' }) {
        Write-Output "NOT RUN [$($remaining.Label)]"
    }
    Write-Output "Verification $($summary.Status) exit $code - summary: $summaryPath"
}
exit $code
