param([string] $PowerShell = (Get-Process -Id $PID).Path)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$check = Join-Path $repo 'scripts/check-verification-environment.ps1'

$output = (& $PowerShell -NoProfile -File $check -MinimumFreeGiB 0 2>&1) -join "`n"
if ($LASTEXITCODE -ne 0 -or $output -notmatch 'PASS verification environment') {
    throw "A working filesystem failed preflight: $output"
}

# Reject insufficient capacity without filling a drive or changing its permissions.
$output = (& $PowerShell -NoProfile -File $check -Repository $repo -MinimumFreeGiB 1000000 2>&1) -join "`n"
if ($LASTEXITCODE -eq 0 -or $output -notmatch 'Insufficient free space' -or $output -notmatch 'GiB') {
    throw "Low-disk preflight did not fail with actionable capacity details: $output"
}

$savedTarget = $env:CARGO_TARGET_DIR
try {
    # Relative target paths resolve from the Rust workspace, as Cargo does.
    $env:CARGO_TARGET_DIR = 'custom-target'
    $output = (& $PowerShell -NoProfile -File $check -Repository $repo -MinimumFreeGiB 0 2>&1) -join "`n"
    if ($LASTEXITCODE -ne 0 -or $output -notmatch 'tricerules[\\/]custom-target') {
        throw "Preflight did not cover the configured Cargo target: $output"
    }
}
finally {
    if ($null -eq $savedTarget) { Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue }
    else { $env:CARGO_TARGET_DIR = $savedTarget }
}
Write-Output 'PASS verification environment regression'
