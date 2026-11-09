# Isolated checkout and native command fixtures for the workflow scripts.
$ErrorActionPreference = 'Stop'
$sourceRepo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$workflowWindows = [IO.Path]::DirectorySeparatorChar -eq '\'

function Assert-Workflow {
    param([bool] $Condition, [string] $Message)
    if (-not $Condition) { throw $Message }
}

function New-WorkflowFixture {
    $root = Join-Path ([IO.Path]::GetTempPath()) ('cockatrice workflow ' + [guid]::NewGuid())
    $fixturePowerShell = (Get-Process -Id $PID).Path
    foreach ($directory in @('scripts', 'tricerules', 'bin', 'nested directory', 'Cockatrice\Cockatrice')) {
        New-Item -ItemType Directory -Path (Join-Path $root $directory) -Force | Out-Null
    }
    foreach ($name in @('run-quiet-command.ps1', 'card-author.ps1', 'gen-card-checklist.ps1', 'verify.ps1', 'routine-ron-checks.ps1', 'update-card-data.ps1', 'prepare-card-batch.ps1', 'focused-test-plan.ps1', 'rust-format-checks.ps1', 'check-rust-format.ps1')) {
        $source = Join-Path $sourceRepo "scripts\$name"
        if (Test-Path -LiteralPath $source) { Copy-Item -LiteralPath $source -Destination (Join-Path $root "scripts\$name") }
    }
    New-Item -ItemType Directory -Path (Join-Path $root 'tests/scripts') -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $root 'scripts/check-verification-environment.ps1') -Value @'
param([int] $MinimumFreeGiB = 10, [switch] $SkipRustCache)
if (Test-Path -LiteralPath (Join-Path $PSScriptRoot '../bad-environment')) { Write-Output "fixture environment failed, minimum free GiB $MinimumFreeGiB"; exit 21 }
if (-not $SkipRustCache -and $env:COCKATRICE_RUST_CACHE_BUDGET_GIB) {
    Write-Output "fixture Rust cache budget $env:COCKATRICE_RUST_CACHE_BUDGET_GIB"
}
Write-Output 'fixture environment passed'
exit 0
'@
    Set-Content -LiteralPath (Join-Path $root 'tests/scripts/card_data_boundary_test.ps1') -Value "Write-Output 'fixture dependency boundary'"
    Copy-Item -LiteralPath (Join-Path $sourceRepo 'tricerules/Cargo.toml') -Destination (Join-Path $root 'tricerules/Cargo.toml')
    foreach ($package in @('tricerules-proto', 'tricerules-core', 'tricerules-card-model', 'tricerules-cards', 'tricerules-server')) {
        $packageRoot = Join-Path $root "tricerules/$package"
        New-Item -ItemType Directory -Path $packageRoot | Out-Null
        Copy-Item -LiteralPath (Join-Path $sourceRepo "tricerules/$package/Cargo.toml") -Destination (Join-Path $packageRoot 'Cargo.toml')
    }
    $formatPackages = @()
    foreach ($package in @('tricerules-proto', 'tricerules-core', 'tricerules-card-model', 'tricerules-cards', 'tricerules-server')) {
        $sourcePath = Join-Path $root "tricerules/$package/lib.rs"
        [IO.File]::WriteAllText($sourcePath, "fn main() {}`n")
        $formatPackages += @{ name = $package; targets = @(@{ src_path = $sourcePath; edition = '2021' }) }
    }
    @{ packages = $formatPackages } | ConvertTo-Json -Depth 8 |
        Set-Content -LiteralPath (Join-Path $root 'format-metadata.json')
    Set-Content -LiteralPath (Join-Path $root 'scripts/check-card-evidence.ps1') -Value @'
param([string] $OracleBulk, [switch] $Preparation, [string] $MapListJson, [string] $MapListFile)
if (-not (Test-Path -LiteralPath $OracleBulk)) { exit 13 }
if (Test-Path -LiteralPath (Join-Path $PSScriptRoot '../bad-evidence')) { Write-Output 'invalid evidence reference'; exit 13 }
if ($MapListFile) { [IO.File]::WriteAllText((Join-Path (Split-Path -Parent $PSScriptRoot) 'selected-map-list.json'), (Get-Content -LiteralPath $MapListFile -Raw)) }
Write-Output 'fixture evidence check'
exit 0
'@
    Set-Content -LiteralPath (Join-Path $root 'oracle-cards.jsonl.gz') -Value 'fixture'
    Set-Content -LiteralPath (Join-Path $root 'oracle-cards.jsonl.gz.meta.json') -Value '{}'
    Set-Content -LiteralPath (Join-Path $root 'cards.xml') -Value '<fixture/>'
    Set-Content -LiteralPath (Join-Path $root 'Cockatrice\Cockatrice\cards.xml') -Value '<fixture/>'
    [IO.File]::WriteAllText((Join-Path $root 'tricerules\CARDS.md'), 'canonical checklist')
    [IO.File]::WriteAllText((Join-Path $root 'fingerprint'), 'canonical')
    $buildStub = @'
$suffix = if ([IO.Path]::DirectorySeparatorChar -eq '\') { '.cmd' } else { '' }
& (Join-Path $PSScriptRoot "../bin/build$suffix") @args
exit $LASTEXITCODE
'@
    foreach ($script in @('build-ninja.ps1', 'build-linux.ps1')) {
        Set-Content -LiteralPath (Join-Path $root "scripts/$script") -Value $buildStub
    }
    Set-Content -LiteralPath (Join-Path $root 'bin\tool.ps1') -Value @'
$ErrorActionPreference = 'Stop'
$tool = $args[0]
$arguments = @($args | Select-Object -Skip 1)
$root = Split-Path -Parent $PSScriptRoot
$joined = $arguments -join ' '
$record = @{ Tool = $tool; Arguments = $arguments; Cwd = (Get-Location).Path; RequireE2E = $env:RULED_E2E_REQUIRE;
    BuildJobs = $env:CARGO_BUILD_JOBS; TestThreads = $env:RUST_TEST_THREADS; Qpa = $env:QT_QPA_PLATFORM;
    Incremental = $env:CARGO_INCREMENTAL; CargoTarget = $env:CARGO_TARGET_DIR; CargoBuildDir = $env:CARGO_BUILD_BUILD_DIR }
($record | ConvertTo-Json -Compress) | Add-Content -LiteralPath (Join-Path $root 'trace.jsonl')
$failure = Join-Path $root 'fail-pattern'
if ((Test-Path -LiteralPath $failure) -and "$tool $joined" -match ([IO.File]::ReadAllText($failure))) {
    Write-Output 'fixture complete failure log'
    exit 7
}
if ($tool -eq 'cargo') {
    if ($arguments[0] -eq 'metadata') {
        Get-Content -LiteralPath (Join-Path $root 'format-metadata.json') -Raw
        exit 0
    }
    if ($arguments -contains '--exact') {
        if (Test-Path -LiteralPath (Join-Path $root 'zero-tests')) {
            Write-Output 'test result: ok. 0 passed; 0 failed; 0 ignored;'
        } else { Write-Output 'test result: ok. 1 passed; 0 failed; 0 ignored;' }
    }
    if ($joined -match '--bin card-author ') {
        if ($arguments -contains 'metadata') {
            if ($arguments -contains '--check') {
                if ([IO.File]::ReadAllText((Join-Path $root 'fingerprint')) -ne 'canonical') { Write-Output 'fingerprint drift'; exit 9 }
            }
            elseif ($arguments -contains '--refresh') { [IO.File]::WriteAllText((Join-Path $root 'fingerprint'), 'canonical') }
            else { throw 'Expected card-author metadata check or refresh' }
        }
    }
    if ($joined -match '--bin gen-checklist ') {
        $target = $null
        for ($i = 0; $i -lt $arguments.Count; $i++) {
            if ($arguments[$i] -eq '--out') { $target = $arguments[$i + 1] }
        }
        [IO.File]::WriteAllText($target, 'canonical checklist')
        if (Test-Path -LiteralPath (Join-Path $root 'bad-names')) { Write-Output 'unmatched card name'; exit 11 }
    }
}
if (Test-Path -LiteralPath (Join-Path $root 'disposable-fixture')) {
    if ($tool -eq 'git' -and $arguments -contains 'rev-parse') {
        if ($arguments -contains '--absolute-git-dir') { Write-Output (Join-Path $root '.git/worktrees/fixture') }
        elseif ($arguments -contains '--git-common-dir') { Write-Output (Join-Path $root '.git') }
        exit 0
    }
    if ($tool -eq 'cargo' -and $arguments[0] -eq 'test') {
        New-Item -ItemType Directory -Path (Join-Path $root 'tricerules/target/debug') -Force | Out-Null
    }
    if ($tool -eq 'build') {
        $preset = if ([IO.Path]::DirectorySeparatorChar -eq '\') { 'windows-ninja-all' } else { 'linux-ninja-all' }
        $output = Join-Path $root "build/$preset"
        New-Item -ItemType Directory -Path $output -Force | Out-Null
        $linkType = if ([IO.Path]::DirectorySeparatorChar -eq '\') { 'Junction' } else { 'SymbolicLink' }
        if ($env:COCKATRICE_TEST_DISPOSABLE_LINK) {
            New-Item -ItemType $linkType -Path (Join-Path $output 'source-link') -Target (Join-Path $root 'tricerules') | Out-Null
        }
    }
}
[Console]::Error.WriteLine('fixture successful stderr')
Write-Output 'fixture successful stdout'
exit 0
'@
    foreach ($tool in @('cargo', 'rustfmt', 'ctest', 'git', 'build', 'cmake')) {
        if (-not $workflowWindows) {
            $shim = Join-Path $root "bin/$tool"
            $shellQuoteEscape = "'" + '"' + "'" + '"' + "'"
            $quotedHost = "'" + $fixturePowerShell.Replace("'", $shellQuoteEscape) + "'"
            $body = '#!/bin/sh' + "`nexec $quotedHost -NoProfile -File " + '"$(dirname "$0")/tool.ps1" ' + $tool + ' "$@"' + "`n"
            [IO.File]::WriteAllText($shim, $body, [Text.UTF8Encoding]::new($false))
            & chmod +x -- $shim
            if ($LASTEXITCODE -ne 0) { throw 'Could not make workflow shim executable.' }
            continue
        }
        Set-Content -LiteralPath (Join-Path $root "bin\$tool.cmd") -Encoding Ascii -Value @(
            '@echo off',
            "`"$fixturePowerShell`" -NoProfile -File `"%~dp0tool.ps1`" $tool %*",
            'exit /b %errorlevel%'
        )
    }
    return $root
}

function Invoke-WorkflowFixture {
    param([string] $Root, [string] $Script, [string[]] $Arguments = @(), [string] $Cwd = $Root)
    $savedPath = $env:PATH
    $savedMixedPath = $env:Path
    $savedLocalAppData = $env:LOCALAPPDATA
    $savedXdgData = $env:XDG_DATA_HOME
    Push-Location $Cwd
    try {
        # Remove both spellings before setting one: this host can supply conflicting copies.
        Remove-Item Env:PATH -ErrorAction SilentlyContinue
        Remove-Item Env:Path -ErrorAction SilentlyContinue
        $env:PATH = "$(Join-Path $Root 'bin')$([IO.Path]::PathSeparator)$savedPath"
        $env:LOCALAPPDATA = $Root
        $env:XDG_DATA_HOME = $Root
        $ErrorActionPreference = 'Continue'
        $output = (& $PowerShell -NoProfile -File (Join-Path $Root "scripts\$Script") @Arguments 2>&1) -join "`n"
        $code = $LASTEXITCODE
        $ErrorActionPreference = 'Stop'
        return [pscustomobject]@{ ExitCode = $code; Output = $output }
    }
    finally {
        Remove-Item Env:PATH -ErrorAction SilentlyContinue
        Remove-Item Env:Path -ErrorAction SilentlyContinue
        $env:PATH = $savedPath
        if ($workflowWindows) { $env:Path = $savedMixedPath }
        $env:LOCALAPPDATA = $savedLocalAppData
        $env:XDG_DATA_HOME = $savedXdgData
        Pop-Location
    }
}

function Read-WorkflowTrace {
    param([string] $Root)
    $path = Join-Path $Root 'trace.jsonl'
    if (Test-Path -LiteralPath $path) { Get-Content -LiteralPath $path | ForEach-Object { $_ | ConvertFrom-Json } }
}

function Remove-WorkflowFixture {
    param([string] $Root)
    $resolved = [IO.Path]::GetFullPath($Root)
    $tempParent = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    if (-not $resolved.StartsWith($tempParent, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove unexpected fixture: $resolved"
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
