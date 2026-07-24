# Configure + build the windows-ninja-all preset inside a VS x64 dev environment.
# Ninja invokes cl.exe directly, so INCLUDE/LIB/PATH from vcvars are needed at both
# configure and build time — this script sets them up so callers don't have to.
#
# Usage:
#   ./scripts/build-ninja.ps1                       # configure (first run) + build everything
#   ./scripts/build-ninja.ps1 --target servatrice   # extra args are passed to cmake --build
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot

$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path $vswhere)) { throw "vswhere.exe not found; is Visual Studio installed?" }
$vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) { throw "No Visual Studio installation with the C++ x64 toolset found." }

# DevShell init shells out to vswhere by bare name; keep its dir on PATH to avoid a noisy warning.
$env:PATH = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer;$env:PATH"
Import-Module (Join-Path $vs 'Common7\Tools\Microsoft.VisualStudio.DevShell.dll')
Enter-VsDevShell -VsInstallPath $vs -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64' | Out-Null

Set-Location $repo
if (-not (Test-Path "$repo\build\windows-ninja-all\build.ninja")) {
    cmake --preset windows-ninja-all
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
cmake --build --preset windows-ninja-all @args
exit $LASTEXITCODE
