<#
.SYNOPSIS
Validate and inspect readable ruled evidence locally, including client report ZIPs.
.EXAMPLE
./scripts/inspect-ruled-capture.ps1 -Capture report.zip -Command cast_spell -Markdown
.EXAMPLE
./scripts/inspect-ruled-capture.ps1 -Capture report.zip -State client -To 120 -Output client-at-120.json
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$Capture,
    [string]$Kind,
    [Alias('Command')][string]$CommandName,
    [Alias('Object')][string]$ObjectId,
    [Nullable[int]]$Seat,
    [string]$Report,
    [uint64]$From = 0,
    [uint64]$To = [uint64]::MaxValue,
    [string]$State,
    [string]$Output,
    [switch]$Markdown,
    [switch]$Validate,
    [switch]$AllowProtocolMismatch
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'ruled-capture-common.ps1')
$taskTool = Get-RuledCaptureTool
$taskInput = Open-RuledCaptureInput -Capture $Capture -Tool $taskTool
$taskExit = 1
try {
    $taskArguments = @('--capture', $taskInput.Directory, '--from', [string]$From, '--to', [string]$To)
    foreach ($taskOption in @(@('kind',$Kind), @('command',$CommandName), @('object',$ObjectId), @('report',$Report), @('state',$State), @('output',$Output))) {
        if ($taskOption[1]) { $taskArguments += @(('--' + $taskOption[0]), [string]$taskOption[1]) }
    }
    if ($null -ne $Seat) { $taskArguments += @('--seat', [string]$Seat) }
    if ($Markdown) { $taskArguments += '--markdown' }
    if ($Validate) { $taskArguments += '--validate' }
    if ($AllowProtocolMismatch) { $taskArguments += '--allow-protocol-mismatch' }
    & $taskTool @taskArguments
    $taskExit = $LASTEXITCODE
} finally { Close-RuledCaptureInput $taskInput.Temporary }
exit $taskExit
