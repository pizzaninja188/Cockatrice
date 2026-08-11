# Run one external command with complete file logging and concise successful output.
# On failure, print the retained log and preserve the command's exact exit code.
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateNotNullOrEmpty()]
    [string] $Label,

    [Parameter(Mandatory)]
    [ValidateNotNullOrEmpty()]
    [string] $Executable,

    [string[]] $ArgumentList = @(),

    [string] $WorkingDirectory,

    [string] $LogDirectory,

    [switch] $ShowLogOnSuccess,

    [switch] $AsResultObject
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot

function ConvertTo-NativePath {
    param(
        [Parameter(Mandatory)]
        [string] $Path
    )

    $providerPrefix = 'Microsoft.PowerShell.Core\FileSystem::'
    if ($Path.StartsWith($providerPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        $Path = $Path.Substring($providerPrefix.Length)
    }

    if ($Path.StartsWith('\\?\UNC\', [System.StringComparison]::OrdinalIgnoreCase)) {
        return '\\' + $Path.Substring(8)
    }
    if ($Path -match '^\\\\\?\\[A-Za-z]:\\') {
        return $Path.Substring(4)
    }

    return $Path
}

if (-not $WorkingDirectory) {
    $WorkingDirectory = $repo
}
elseif (-not [System.IO.Path]::IsPathRooted($WorkingDirectory)) {
    $WorkingDirectory = Join-Path $repo $WorkingDirectory
}
$resolvedWorkingDirectory = ConvertTo-NativePath (Resolve-Path -LiteralPath $WorkingDirectory -ErrorAction Stop).Path

if (-not $LogDirectory) {
    $LogDirectory = Join-Path $repo 'build\verification-logs'
}
elseif (-not [System.IO.Path]::IsPathRooted($LogDirectory)) {
    $LogDirectory = Join-Path $repo $LogDirectory
}
New-Item -ItemType Directory -Path $LogDirectory -Force | Out-Null
$resolvedLogDirectory = ConvertTo-NativePath (Resolve-Path -LiteralPath $LogDirectory -ErrorAction Stop).Path

$safeLabel = ($Label -replace '[^A-Za-z0-9._-]+', '-').Trim('-')
if (-not $safeLabel) {
    $safeLabel = 'command'
}
$timestamp = Get-Date -Format 'yyyyMMdd-HHmmss-fff'
$logPath = Join-Path $resolvedLogDirectory "$timestamp-$safeLabel.log"

$exitCode = 1
Push-Location $resolvedWorkingDirectory
try {
    try {
        $nativeExitCode = $null
        try {
            $ErrorActionPreference = 'Continue'
            $global:LASTEXITCODE = $null
            & $Executable @ArgumentList *> $logPath
            $nativeExitCode = $LASTEXITCODE
        }
        finally {
            $ErrorActionPreference = 'Stop'
        }

        if ($null -eq $nativeExitCode) {
            throw "External command did not report an exit code: $Executable"
        }
        $exitCode = $nativeExitCode
    }
    catch {
        $_ | Out-String | Set-Content -LiteralPath $logPath
        $exitCode = 1
    }
}
finally {
    Pop-Location
}

$status = if ($exitCode -eq 0) { 'PASS' } else { 'FAIL' }
$summary = "$status [$Label] exit $exitCode - log: $logPath"
$showLog = ($exitCode -ne 0) -or $ShowLogOnSuccess
$result = [pscustomobject]@{
    Label = $Label
    ExitCode = $exitCode
    LogPath = $logPath
    ShowLog = $showLog
    Summary = $summary
}

if ($AsResultObject) {
    return $result
}

Write-Output $summary
if ($showLog -and (Test-Path -LiteralPath $logPath -PathType Leaf)) {
    Get-Content -LiteralPath $logPath
}
exit $exitCode
