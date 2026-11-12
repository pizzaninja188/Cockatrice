$ErrorActionPreference = 'Stop'

function Assert-True {
    param(
        [Parameter(Mandatory)]
        [bool] $Condition,

        [Parameter(Mandatory)]
        [string] $Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$pruner = Join-Path $repo 'scripts\prune-evidence.ps1'
$root = Join-Path ([System.IO.Path]::GetTempPath()) ("cockatrice-prune-evidence-test-" + [guid]::NewGuid())
$build = Join-Path $root 'build'
$campaign = Join-Path $build 'campaign'
$logs = Join-Path $build 'verification-logs'
$old = [DateTime]::UtcNow.AddDays(-40)

function Set-OldTimestamp {
    param([string] $Path)
    Get-ChildItem -LiteralPath $Path -Recurse -Force -ErrorAction SilentlyContinue |
        ForEach-Object { $_.LastWriteTimeUtc = $old }
    (Get-Item -LiteralPath $Path -Force).LastWriteTimeUtc = $old
}

function New-EvidenceFile {
    param([string] $Path, [string] $Value = 'evidence')
    New-Item -ItemType Directory -Path (Split-Path -Parent $Path) -Force | Out-Null
    Set-Content -LiteralPath $Path -Value $Value
}

try {
    foreach ($directory in @($campaign, $logs)) { New-Item -ItemType Directory -Path $directory -Force | Out-Null }

    $oldReport = Join-Path $campaign 'old.report.json'
    $freshReport = Join-Path $campaign 'fresh.report.json'
    New-EvidenceFile $oldReport
    New-EvidenceFile $freshReport

    $oldLog = Join-Path $logs 'old-command.log'
    New-EvidenceFile $oldLog

    $completedRun = Join-Path $logs 'verify-completed'
    New-EvidenceFile (Join-Path $completedRun 'frames.pb')
    New-EvidenceFile (Join-Path $completedRun 'summary.json') '{"Status":"Pass","CompletedAt":"2026-01-01T00:00:00Z"}'

    $liveRun = Join-Path $logs 'verify-live'
    New-EvidenceFile (Join-Path $liveRun 'frames.pb')
    New-EvidenceFile (Join-Path $liveRun 'summary.json') '{"Status":"Running","CompletedAt":null}'

    $baselineRun = Join-Path $logs 'verify-baseline'
    New-EvidenceFile (Join-Path $baselineRun 'run.log')
    New-EvidenceFile (Join-Path $baselineRun 'routine-inputs.json') '{"Version":1}'
    New-EvidenceFile (Join-Path $baselineRun 'summary.json') '{"Status":"Pass","CompletedAt":"2026-01-01T00:00:00Z"}'

    $archive = Join-Path $logs 'verify-archived.zip'
    New-EvidenceFile $archive 'archive'

    $buildOutput = Join-Path $build 'windows-ninja-all\keep.txt'
    New-EvidenceFile $buildOutput

    Get-ChildItem -LiteralPath $logs -Directory -Force | ForEach-Object { Set-OldTimestamp $_.FullName }
    Set-OldTimestamp $oldReport
    Set-OldTimestamp $oldLog
    Set-OldTimestamp $archive
    Set-OldTimestamp $buildOutput

    & $pruner -Days 14 -BuildRoot $build -Quiet
    Assert-True ($LASTEXITCODE -eq 0) 'Dry run failed.'
    Assert-True (Test-Path -LiteralPath $oldReport) 'Dry run deleted a campaign report.'
    Assert-True (Test-Path -LiteralPath (Join-Path $completedRun 'frames.pb')) 'Dry run deleted capture frames.'

    & $pruner -Days 14 -BuildRoot $build -Apply -Quiet
    Assert-True ($LASTEXITCODE -eq 0) 'Prune run failed.'

    Assert-True (-not (Test-Path -LiteralPath $oldReport)) 'Expired campaign report was not removed.'
    Assert-True (Test-Path -LiteralPath $freshReport) 'Fresh campaign report was removed.'
    Assert-True (-not (Test-Path -LiteralPath $oldLog)) 'Expired verification log was not removed.'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $completedRun 'frames.pb'))) 'Expired capture frames were not removed.'
    Assert-True (Test-Path -LiteralPath (Join-Path $completedRun 'summary.json')) 'Completed verification summary was removed.'
    Assert-True (Test-Path -LiteralPath (Join-Path $liveRun 'frames.pb')) 'An incomplete verification run was pruned.'
    Assert-True (Test-Path -LiteralPath (Join-Path $liveRun 'summary.json')) 'An incomplete verification summary was pruned.'
    Assert-True (Test-Path -LiteralPath (Join-Path $baselineRun 'run.log')) 'A run holding routine inputs was pruned.'
    Assert-True (Test-Path -LiteralPath (Join-Path $baselineRun 'routine-inputs.json')) 'Protected routine inputs were removed.'
    Assert-True (Test-Path -LiteralPath $archive) 'An archived evidence bundle was pruned.'
    Assert-True (Test-Path -LiteralPath $buildOutput) 'A build directory outside the evidence roots was touched.'

    $disabled = Join-Path $campaign 'disabled.report.json'
    New-EvidenceFile $disabled
    Set-OldTimestamp $disabled
    & $pruner -Days 0 -BuildRoot $build -Apply -Quiet
    Assert-True ($LASTEXITCODE -eq 0) 'Disabled retention failed.'
    Assert-True (Test-Path -LiteralPath $disabled) 'Disabled retention removed an item.'

    Write-Output 'PASS evidence retention'
}
finally {
    if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
}
