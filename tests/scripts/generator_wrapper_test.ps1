param([string] $PowerShell = (Get-Process -Id $PID).Path)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('cockatrice-generator-wrapper-' + [guid]::NewGuid())
$originalPath = $env:PATH
$originalMixedPath = $env:Path
New-Item -ItemType Directory -Path $fixture | Out-Null
try {
    $env:PATH = "$fixture;$originalPath"
    foreach ($expected in @(7, 0)) {
        Set-Content -LiteralPath (Join-Path $fixture 'cargo.cmd') -Encoding Ascii -Value @(
            '@echo generator-stdout',
            '@echo generator-stderr 1>&2',
            "@exit /b $expected"
        )
        foreach ($script in @('gen-cards.ps1', 'gen-card-checklist.ps1')) {
            $arguments = @('-NoProfile', '-File', (Join-Path $repo "scripts\$script"))
            if ($script -eq 'gen-cards.ps1') { $arguments += @('--input', 'fixture-input') }
            # Native stderr is diagnostic output, not the command's success status.
            $ErrorActionPreference = 'Continue'
            $output = (& $PowerShell @arguments 2>&1) -join "`n"
            $actual = $LASTEXITCODE
            $ErrorActionPreference = 'Stop'
            if ($actual -ne $expected) {
                throw "$script returned $actual instead of $expected.`n$output"
            }
            if ($output -notmatch 'generator-stdout' -or $output -notmatch 'generator-stderr') {
                throw "$script lost generator output: $output"
            }
        }
    }
    Remove-Item Env:PATH -ErrorAction SilentlyContinue
    Remove-Item Env:Path -ErrorAction SilentlyContinue
    $env:Path = Join-Path $env:SystemRoot 'System32'
    foreach ($script in @('gen-cards.ps1', 'gen-card-checklist.ps1')) {
        $arguments = @('-NoProfile', '-File', (Join-Path $repo "scripts\$script"))
        if ($script -eq 'gen-cards.ps1') { $arguments += @('--input', 'fixture-input') }
        $ErrorActionPreference = 'Continue'
        $output = (& $PowerShell @arguments 2>&1) -join "`n"
        $actual = $LASTEXITCODE
        $ErrorActionPreference = 'Stop'
        if ($actual -eq 0) { throw "$script reported success without cargo.`n$output" }
    }
}
finally {
    Remove-Item Env:PATH -ErrorAction SilentlyContinue
    Remove-Item Env:Path -ErrorAction SilentlyContinue
    $env:PATH = $originalPath
    $env:Path = $originalMixedPath
    $resolved = [IO.Path]::GetFullPath($fixture)
    $tempParent = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if (-not $resolved.StartsWith($tempParent, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove unexpected fixture: $resolved"
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
Write-Output 'PASS generator wrapper regression'
