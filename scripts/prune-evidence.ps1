<#
.SYNOPSIS
    Prune expired build evidence while retaining completed verification summaries.
.DESCRIPTION
    Scans build/campaign and build/verification-logs for items whose newest write is older than
    -Days. Completed verify-* runs keep summary.json; incomplete runs, routine-inputs.json
    baselines, linked paths and .zip archives are retained. The script is a dry run unless
    -Apply is supplied. -Days 0 disables retention.
.EXAMPLE
    ./scripts/prune-evidence.ps1 -Days 14
.EXAMPLE
    ./scripts/prune-evidence.ps1 -Days 14 -Apply
#>
[CmdletBinding()]
param(
    [ValidateRange(0, 3650)]
    [int] $Days = 14,
    [switch] $Apply,
    [switch] $Quiet,
    [string] $BuildRoot
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (-not $BuildRoot) { $BuildRoot = Join-Path $repo 'build' }
$BuildRoot = [IO.Path]::GetFullPath($BuildRoot)

if ($Days -eq 0) {
    if (-not $Quiet) { Write-Output 'Evidence retention is disabled.' }
    exit 0
}

$cutoff = [DateTime]::UtcNow.AddDays(-$Days)

function Get-NewestWriteTimeUtc {
    param([string] $Path)
    $newest = (Get-Item -LiteralPath $Path -Force).LastWriteTimeUtc
    $entries = @(Get-ChildItem -LiteralPath $Path -Recurse -Force -ErrorAction SilentlyContinue)
    if ($entries.Count) {
        $observed = ($entries | Measure-Object -Property LastWriteTimeUtc -Maximum).Maximum
        if ($observed -gt $newest) { $newest = $observed }
    }
    return $newest
}

function Get-TreeBytes {
    param([string] $Path)
    $item = Get-Item -LiteralPath $Path -Force
    if (-not $item.PSIsContainer) { return [long] $item.Length }
    $sum = (Get-ChildItem -LiteralPath $Path -Recurse -File -Force -ErrorAction SilentlyContinue |
        Measure-Object -Property Length -Sum).Sum
    if ($null -eq $sum) { return 0 }
    return [long] $sum
}

function Test-ProtectedInput {
    param([string] $Path, [bool] $IsContainer)
    if (-not $IsContainer) { return ([IO.Path]::GetFileName($Path) -ieq 'routine-inputs.json') }
    return [bool] (Get-ChildItem -LiteralPath $Path -Recurse -File -Force -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -ieq 'routine-inputs.json' } | Select-Object -First 1)
}

function Test-CompletedVerification {
    param([string] $Path)
    $summaryPath = Join-Path $Path 'summary.json'
    if (-not (Test-Path -LiteralPath $summaryPath -PathType Leaf)) { return $false }
    try { $summary = Get-Content -LiteralPath $summaryPath -Raw | ConvertFrom-Json }
    catch { return $false }
    return ($summary.Status -in @('Pass', 'Fail') -and [bool] $summary.CompletedAt)
}

function Remove-EvidenceTree {
    param([string] $Path)
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
    if (-not $item) { return }
    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        if ($item.PSIsContainer) { [IO.Directory]::Delete($Path) }
        else { [IO.File]::Delete($Path) }
        return
    }
    if ($item.PSIsContainer) {
        foreach ($child in @(Get-ChildItem -LiteralPath $Path -Force)) { Remove-EvidenceTree $child.FullName }
        [IO.Directory]::Delete($Path)
        return
    }
    if ($item.IsReadOnly) { $item.IsReadOnly = $false }
    [IO.File]::Delete($Path)
}

function Remove-ExpiredVerificationRun {
    param([string] $Path)
    foreach ($child in @(Get-ChildItem -LiteralPath $Path -Force)) {
        if (-not $child.PSIsContainer -and $child.Name -ieq 'summary.json') { continue }
        Remove-EvidenceTree $child.FullName
    }
}

$pruned = 0
$summariesKept = 0
$skipped = 0
$bytes = [long] 0

foreach ($name in @('campaign', 'verification-logs')) {
    $root = Join-Path $BuildRoot $name
    if (-not (Test-Path -LiteralPath $root -PathType Container)) { continue }
    foreach ($item in @(Get-ChildItem -LiteralPath $root -Force | Sort-Object Name)) {
        $label = $item.FullName.Substring($BuildRoot.Length + 1)
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            $skipped++
            if (-not $Quiet) { Write-Output "SKIP linked evidence: $label" }
            continue
        }
        if (-not $item.PSIsContainer -and $item.Extension -ieq '.zip') {
            $skipped++
            continue
        }
        try {
            if ((Get-NewestWriteTimeUtc $item.FullName) -ge $cutoff) { continue }
            if (Test-ProtectedInput $item.FullName $item.PSIsContainer) {
                $skipped++
                if (-not $Quiet) { Write-Output "SKIP protected baseline: $label" }
                continue
            }
            $isVerificationRun = $item.PSIsContainer -and $item.Name -like 'verify-*'
            if ($isVerificationRun -and -not (Test-CompletedVerification $item.FullName)) {
                $skipped++
                if (-not $Quiet) { Write-Output "SKIP incomplete verification run: $label" }
                continue
            }
            $keepSummary = $isVerificationRun
            $size = Get-TreeBytes $item.FullName
            if (-not $Apply) {
                $mode = if ($keepSummary) { 'keep summary.json' } else { 'remove' }
                if (-not $Quiet) { Write-Output ("WOULD PRUNE {0} ({1:N1} MB, {2})" -f $label, ($size / 1MB), $mode) }
                continue
            }
            if ($keepSummary) {
                Remove-ExpiredVerificationRun $item.FullName
                $summariesKept++
                if (-not $Quiet) { Write-Output ("PRUNED {0} ({1:N1} MB, kept summary.json)" -f $label, ($size / 1MB)) }
            }
            else {
                Remove-EvidenceTree $item.FullName
                if (-not $Quiet) { Write-Output ("PRUNED {0} ({1:N1} MB)" -f $label, ($size / 1MB)) }
            }
            $pruned++
            $bytes += $size
        }
        catch {
            $skipped++
            if (-not $Quiet) { Write-Warning "Could not prune ${label}: $_" }
        }
    }
}

if (-not $Quiet) {
    $action = if ($Apply) { 'removed' } else { 'would remove' }
    Write-Output ("Evidence retention ({0} days): {1} {2} item(s), {3:N1} MB; kept {4} completed summary file(s); skipped {5}." -f
        $Days, $action, $pruned, ($bytes / 1MB), $summariesKept, $skipped)
}
exit 0
