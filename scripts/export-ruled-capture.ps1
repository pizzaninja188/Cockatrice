<#
.SYNOPSIS
Export server-only evidence by report/capture ID from a maintainer-owned capture root.
.DESCRIPTION
Requires local filesystem access to the server capture. An ID grants no download authority.
The ZIP contains hidden zones and every deck; review it before sharing with a trusted maintainer.
.EXAMPLE
./scripts/export-ruled-capture.ps1 -CaptureRoot D:\server\captures\server -Id <report-id> -Output full-capture.zip
#>
[CmdletBinding(DefaultParameterSetName='Find')]
param(
    [Parameter(Mandatory=$true, ParameterSetName='Find')][string]$CaptureRoot,
    [Parameter(Mandatory=$true, ParameterSetName='Find')][string]$Id,
    [Parameter(Mandatory=$true, ParameterSetName='Direct')][string]$Capture,
    [Parameter(Mandatory=$true)][string]$Output
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'ruled-capture-common.ps1')
$taskTool = Get-RuledCaptureTool
if ($PSCmdlet.ParameterSetName -eq 'Find') {
    if ($Id -notmatch '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$') { throw 'Expected a report or capture UUID.' }
    $taskMatches = @()
    foreach ($taskDirectory in Get-ChildItem -LiteralPath $CaptureRoot -Directory -ErrorAction Stop) {
        if ($taskDirectory.Attributes -band [IO.FileAttributes]::ReparsePoint) { continue }
        if ($taskDirectory.Name -notmatch '^[0-9a-fA-F-]{36}$') { continue }
        $taskManifestPath = Join-Path $taskDirectory.FullName 'manifest.json'
        if (-not (Test-Path -LiteralPath $taskManifestPath -PathType Leaf)) { continue }
        if ((Get-Item -LiteralPath $taskManifestPath).Length -gt 1048576) { continue }
        try { $taskManifest = Get-Content -LiteralPath $taskManifestPath -Raw | ConvertFrom-Json } catch { continue }
        if ($taskManifest.source -eq 'server' -and $taskManifest.privacy -eq 'server_only' -and
            ($taskManifest.capture_id -eq $Id -or $taskManifest.report_ids -contains $Id)) { $taskMatches += $taskDirectory.FullName }
    }
    if ($taskMatches.Count -ne 1) { throw "Expected one matching server capture; found $($taskMatches.Count). Use -Capture for an exact directory." }
    $Capture = $taskMatches[0]
}
$taskInput = Open-RuledCaptureInput -Capture $Capture -Tool $taskTool
$taskExit = 1
try {
    $taskManifest = Get-Content -LiteralPath (Join-Path $taskInput.Directory 'manifest.json') -Raw | ConvertFrom-Json
    if ($taskManifest.source -ne 'server' -or $taskManifest.privacy -ne 'server_only') { throw 'Maintainer export requires a server-only capture.' }
    & $taskTool --capture $taskInput.Directory --pack $Output
    $taskExit = $LASTEXITCODE
} finally { Close-RuledCaptureInput $taskInput.Temporary }
exit $taskExit
