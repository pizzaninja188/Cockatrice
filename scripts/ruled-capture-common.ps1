# Shared local CLI routing. No network calls or uploads.
function Get-RuledCaptureTool {
    $taskRepo = Split-Path -Parent $PSScriptRoot
    $taskTool = Join-Path $taskRepo 'build\windows-ninja-all\libcockatrice_protocol\ruled-capture-tool.exe'
    if (-not (Test-Path -LiteralPath $taskTool -PathType Leaf)) {
        throw 'Build the capture tools first: ./scripts/build-ninja.ps1 --target ruled-capture-tool'
    }
    return $taskTool
}
function Open-RuledCaptureInput {
    param([Parameter(Mandatory=$true)][string]$Capture, [Parameter(Mandatory=$true)][string]$Tool)
    $taskInput = (Resolve-Path -LiteralPath $Capture -ErrorAction Stop).Path
    if (Test-Path -LiteralPath $taskInput -PathType Container) { return @{ Directory = $taskInput; Temporary = $null } }
    if ([IO.Path]::GetFileName($taskInput) -eq 'manifest.json') { return @{ Directory = (Split-Path -Parent $taskInput); Temporary = $null } }
    if ([IO.Path]::GetExtension($taskInput) -ne '.zip') { throw 'Capture must be a directory, manifest.json, or diagnostic ZIP.' }
    $taskTemporary = Join-Path ([IO.Path]::GetTempPath()) ('ruled-capture-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $taskTemporary -ErrorAction Stop | Out-Null
    & $Tool --capture $taskInput --unpack $taskTemporary | Out-Host
    if ($LASTEXITCODE -ne 0) { Close-RuledCaptureInput $taskTemporary; throw 'Diagnostic archive validation failed.' }
    return @{ Directory = $taskTemporary; Temporary = $taskTemporary }
}
function Close-RuledCaptureInput {
    param([string]$Temporary)
    if (-not $Temporary) { return }
    $taskResolved = [IO.Path]::GetFullPath($Temporary)
    $taskTempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/') + [IO.Path]::DirectorySeparatorChar
    if (-not $taskResolved.StartsWith($taskTempRoot, [StringComparison]::OrdinalIgnoreCase) -or
        [IO.Path]::GetFileName($taskResolved) -notmatch '^ruled-capture-[0-9a-f]{32}$') {
        throw 'Refusing to remove an unexpected extraction path.'
    }
    if (Test-Path -LiteralPath $taskResolved) {
        if ((Get-Item -LiteralPath $taskResolved).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Refusing to remove a linked extraction directory.' }
        Remove-Item -LiteralPath $taskResolved -Recurse -Force -ErrorAction Stop
    }
}
