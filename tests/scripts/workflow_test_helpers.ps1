# Isolated checkout and native command fixtures for the workflow scripts.
$ErrorActionPreference = 'Stop'
$sourceRepo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)

function Assert-Workflow {
    param([bool] $Condition, [string] $Message)
    if (-not $Condition) { throw $Message }
}

function New-WorkflowFixture {
    $root = Join-Path ([IO.Path]::GetTempPath()) ('cockatrice workflow ' + [guid]::NewGuid())
    foreach ($directory in @('scripts', 'tricerules', 'bin', 'nested directory', 'Cockatrice\Cockatrice')) {
        New-Item -ItemType Directory -Path (Join-Path $root $directory) -Force | Out-Null
    }
    foreach ($name in @('run-quiet-command.ps1', 'gen-cards.ps1', 'gen-card-checklist.ps1', 'verify.ps1', 'update-card-data.ps1')) {
        $source = Join-Path $sourceRepo "scripts\$name"
        if (Test-Path -LiteralPath $source) { Copy-Item -LiteralPath $source -Destination (Join-Path $root "scripts\$name") }
    }
    Set-Content -LiteralPath (Join-Path $root 'oracle-cards.jsonl.gz') -Value 'fixture'
    Set-Content -LiteralPath (Join-Path $root 'oracle-cards.jsonl.gz.meta.json') -Value '{}'
    Set-Content -LiteralPath (Join-Path $root 'cards.xml') -Value '<fixture/>'
    Set-Content -LiteralPath (Join-Path $root 'Cockatrice\Cockatrice\cards.xml') -Value '<fixture/>'
    [IO.File]::WriteAllText((Join-Path $root 'tricerules\CARDS.md'), 'canonical checklist')
    [IO.File]::WriteAllText((Join-Path $root 'fingerprint'), 'canonical')
    Set-Content -LiteralPath (Join-Path $root 'scripts\build-ninja.ps1') -Value @'
& (Join-Path $PSScriptRoot '..\bin\build.cmd') @args
exit $LASTEXITCODE
'@
    Set-Content -LiteralPath (Join-Path $root 'bin\tool.ps1') -Value @'
$ErrorActionPreference = 'Stop'
$tool = $args[0]
$arguments = @($args | Select-Object -Skip 1)
$root = Split-Path -Parent $PSScriptRoot
$joined = $arguments -join ' '
$record = @{ Tool = $tool; Arguments = $arguments; Cwd = (Get-Location).Path; RequireE2E = $env:RULED_E2E_REQUIRE }
($record | ConvertTo-Json -Compress) | Add-Content -LiteralPath (Join-Path $root 'trace.jsonl')
$failure = Join-Path $root 'fail-pattern'
if ((Test-Path -LiteralPath $failure) -and "$tool $joined" -match ([IO.File]::ReadAllText($failure))) {
    Write-Output 'fixture complete failure log'
    exit 7
}
if ($tool -eq 'cargo') {
    if ($joined -match '--bin gen-cards ') {
        if ($arguments -contains '--include-new') { throw 'Unexpected bulk expansion' }
        $fingerprint = Join-Path $root 'fingerprint'
        if ($arguments -contains '--check') {
            if ([IO.File]::ReadAllText($fingerprint) -ne 'canonical') { Write-Output 'fingerprint drift'; exit 9 }
        }
        elseif ($arguments -notcontains '--dry-run') { [IO.File]::WriteAllText($fingerprint, 'canonical') }
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
[Console]::Error.WriteLine('fixture successful stderr')
Write-Output 'fixture successful stdout'
exit 0
'@
    foreach ($tool in @('cargo', 'ctest', 'git', 'build')) {
        Set-Content -LiteralPath (Join-Path $root "bin\$tool.cmd") -Encoding Ascii -Value @(
            '@echo off',
            "powershell.exe -NoProfile -File `"%~dp0tool.ps1`" $tool %*",
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
    Push-Location $Cwd
    try {
        # Remove both spellings before setting one: this host can supply conflicting copies.
        Remove-Item Env:PATH -ErrorAction SilentlyContinue
        Remove-Item Env:Path -ErrorAction SilentlyContinue
        $env:Path = "$(Join-Path $Root 'bin');$savedPath"
        $env:LOCALAPPDATA = $Root
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
        $env:Path = $savedMixedPath
        $env:LOCALAPPDATA = $savedLocalAppData
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
    $tempParent = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if (-not $resolved.StartsWith($tempParent, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove unexpected fixture: $resolved"
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
