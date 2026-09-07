<#
.SYNOPSIS
Reconstruct a maintainer capture through the same engine session used by the live sidecar.
.EXAMPLE
./scripts/replay-ruled-capture.ps1 -Capture full-capture.zip -StopBefore 24 -Output build/before-24
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$Capture,
    [Nullable[uint64]]$StopAfter,
    [Nullable[uint64]]$StopBefore,
    [Nullable[uint64]]$RetrySequence,
    [string]$Output,
    [switch]$AllowBuildMismatch
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'ruled-capture-common.ps1')
$taskSelections = @('StopAfter','StopBefore','RetrySequence' | Where-Object { $PSBoundParameters.ContainsKey($_) })
if ($taskSelections.Count -gt 1) { throw 'Choose one stop boundary or retry sequence.' }
$taskTool = Get-RuledCaptureTool
$taskInput = Open-RuledCaptureInput -Capture $Capture -Tool $taskTool
$taskExit = 1
try {
    $taskReplay = Join-Path (Split-Path -Parent $PSScriptRoot) 'tricerules\target\release\tricerules-replay.exe'
    if (-not (Test-Path -LiteralPath $taskReplay)) { throw 'Build tricerules-replay first with scripts/build-ninja.ps1.' }
    $taskArguments = @('--capture', $taskInput.Directory)
    if ($null -ne $StopAfter) { $taskArguments += @('--stop-after', $StopAfter.ToString()) }
    if ($null -ne $StopBefore) { $taskArguments += @('--stop-before', $StopBefore.ToString()) }
    if ($null -ne $RetrySequence) { $taskArguments += @('--retry-sequence', $RetrySequence.ToString()) }
    if ($Output) { $taskArguments += @('--output', $Output) }
    if ($AllowBuildMismatch) { $taskArguments += '--allow-build-mismatch' }
    & $taskReplay @taskArguments
    $taskExit = $LASTEXITCODE
} finally { Close-RuledCaptureInput $taskInput.Temporary }
exit $taskExit
