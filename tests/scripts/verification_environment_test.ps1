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

$fixture = Join-Path $repo ('build/storage-test-' + [guid]::NewGuid())
$workspace = Join-Path $fixture 'tricerules'
$debug = Join-Path $workspace 'target/debug'
$release = Join-Path $workspace 'target/release'
$savedBudget = $env:COCKATRICE_RUST_CACHE_BUDGET_GIB
$savedBuildDirectory = $env:CARGO_BUILD_BUILD_DIR
try {
    Remove-Item Env:CARGO_TARGET_DIR,Env:CARGO_BUILD_BUILD_DIR -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Path $debug, $release, (Join-Path $workspace 'src') -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $workspace 'Cargo.toml') -Value @'
[package]
name = "storage-fixture"
version = "0.1.0"
edition = "2021"
'@
    Set-Content -LiteralPath (Join-Path $workspace 'src/lib.rs') -Value 'pub fn fixture() {}'
    Set-Content -LiteralPath (Join-Path $workspace 'target/CACHEDIR.TAG') -Value 'Signature: 8a477f597d28d172789f06886806bc55' -Encoding Ascii
    Set-Content -LiteralPath (Join-Path $debug 'test.pdb') -Value ('x' * 4096)
    Set-Content -LiteralPath (Join-Path $release 'sidecar') -Value 'release'

    $env:COCKATRICE_RUST_CACHE_BUDGET_GIB = '1'
    $output = (& $PowerShell -NoProfile -File $check -Repository $fixture -MinimumFreeGiB 0 2>&1) -join "`n"
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $debug)) {
        throw "Preflight discarded a cache within budget: $output"
    }

    $env:COCKATRICE_RUST_CACHE_BUDGET_GIB = '0.000001'
    $env:CARGO_TARGET_DIR = 'redirected-target'
    $output = (& $PowerShell -NoProfile -File $check -Repository $fixture -MinimumFreeGiB 0 2>&1) -join "`n"
    if ($LASTEXITCODE -eq 0 -or $output -notmatch 'default Cargo target' -or -not (Test-Path -LiteralPath $debug)) {
        throw "Managed preflight did not reject a redirected target: $output"
    }
    Remove-Item Env:CARGO_TARGET_DIR
    $env:CARGO_BUILD_BUILD_DIR = 'redirected-build'
    $output = (& $PowerShell -NoProfile -File $check -Repository $fixture -MinimumFreeGiB 0 2>&1) -join "`n"
    if ($LASTEXITCODE -eq 0 -or $output -notmatch 'default Cargo target and build' -or -not (Test-Path -LiteralPath $debug)) {
        throw "Managed preflight did not reject a redirected build directory: $output"
    }
    Remove-Item Env:CARGO_BUILD_BUILD_DIR

    $output = (& $PowerShell -NoProfile -File $check -Repository $fixture -MinimumFreeGiB 0 -SkipRustCache 2>&1) -join "`n"
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $debug)) {
        throw "A non-Rust preflight did not bypass cache maintenance: $output"
    }
    $output = (& $PowerShell -NoProfile -File $check -Repository $fixture -MinimumFreeGiB 0 2>&1) -join "`n"
    if ($LASTEXITCODE -ne 0 -or (Test-Path -LiteralPath $debug) -or -not (Test-Path -LiteralPath (Join-Path $release 'sidecar'))) {
        throw "Managed preflight did not recycle only the dev profile: $output"
    }
}
finally {
    if ($null -eq $savedBudget) { Remove-Item Env:COCKATRICE_RUST_CACHE_BUDGET_GIB -ErrorAction SilentlyContinue }
    else { $env:COCKATRICE_RUST_CACHE_BUDGET_GIB = $savedBudget }
    if ($null -eq $savedTarget) { Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue }
    else { $env:CARGO_TARGET_DIR = $savedTarget }
    $env:CARGO_BUILD_BUILD_DIR = $savedBuildDirectory
    $resolvedFixture = (Resolve-Path -LiteralPath $fixture).ProviderPath
    if ($resolvedFixture -cne [IO.Path]::GetFullPath($fixture)) { throw "Unexpected test path: $resolvedFixture" }
    Remove-Item -LiteralPath $resolvedFixture -Recurse -Force
}
Write-Output 'PASS verification Rust cache budget regression'
