param(
    [Parameter(Mandatory = $true)]
    [string]$OracleExecutable,
    [string]$QtVersionName = 'Qt6',
    [string]$Configuration = 'Release'
)

$ErrorActionPreference = 'Stop'
$oraclePath = (Resolve-Path -LiteralPath $OracleExecutable).Path
$oracleDirectory = Split-Path -Parent $oraclePath
$suffix = if ($Configuration -eq 'Debug') { 'd' } else { '' }
foreach ($relativePath in @("${QtVersionName}Core${suffix}.dll", "${QtVersionName}Gui${suffix}.dll",
                            "${QtVersionName}Widgets${suffix}.dll", "platforms/qwindows${suffix}.dll")) {
    if (-not (Test-Path -LiteralPath (Join-Path $oracleDirectory $relativePath))) {
        throw "Oracle runtime is missing $relativePath beside $oraclePath"
    }
}

# Exercise QApplication startup without importing data or opening the wizard. An unknown option
# exits through QCommandLineParser before settings are accessed. Isolate DLL/plugin discovery so
# the development Qt installation cannot mask an incomplete app-local deployment.
$startInfo = New-Object System.Diagnostics.ProcessStartInfo
$startInfo.FileName = $oraclePath
$startInfo.Arguments = '--runtime-dependency-probe'
$startInfo.WorkingDirectory = $oracleDirectory
$startInfo.UseShellExecute = $false
$startInfo.CreateNoWindow = $true
$startInfo.RedirectStandardOutput = $true
$startInfo.RedirectStandardError = $true
foreach ($key in @($startInfo.EnvironmentVariables.Keys)) {
    if ($key -ieq 'PATH' -or $key -like 'QT_*') {
        $startInfo.EnvironmentVariables.Remove($key)
    }
}
$startInfo.EnvironmentVariables['PATH'] = "$env:SystemRoot\System32;$env:SystemRoot"
$startInfo.EnvironmentVariables['QT_QPA_PLATFORM'] = 'windows'
$startInfo.EnvironmentVariables['QT_COMMAND_LINE_PARSER_NO_GUI_MESSAGE_BOXES'] = '1'
$process = New-Object System.Diagnostics.Process
$process.StartInfo = $startInfo
try {
    [void]$process.Start()
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit(10000)) {
        $process.Kill()
        $process.WaitForExit()
        throw 'Oracle startup probe timed out.'
    }
    $output = $stdout.GetAwaiter().GetResult() + $stderr.GetAwaiter().GetResult()
    if ($process.ExitCode -ne 1 -or $output -notmatch "Unknown option 'runtime-dependency-probe'") {
        throw "Oracle did not reach command-line parsing (exit $($process.ExitCode)): $output"
    }
    Write-Output 'PASS: Oracle starts with its deployed DLLs and Qt platform plugin.'
} finally {
    $process.Dispose()
}
