# Configure and build the full ruled tree using system Qt/development packages.
# PowerShell 7: ./scripts/build-linux.ps1 [--target <target>]
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Push-Location $repo
try {
    cmake --preset linux-ninja-all
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    cmake --build --preset linux-ninja-all @args
    exit $LASTEXITCODE
}
finally { Pop-Location }
