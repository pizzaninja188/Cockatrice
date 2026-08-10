$ErrorActionPreference = 'Stop'

function Assert-True {
    param(
        [Parameter(Mandatory)]
        [bool] $Condition,

        [Parameter(Mandatory)]
        [string] $Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$runner = Join-Path $repo 'scripts\run-quiet-command.ps1'
if (-not (Test-Path -LiteralPath $runner -PathType Leaf)) {
    throw "Quiet command runner not found: $runner"
}

$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("cockatrice-quiet-command-test-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $tempRoot | Out-Null

try {
    $success = & $runner `
        -Label 'successful fixture' `
        -Executable $env:ComSpec `
        -ArgumentList @('/d', '/c', 'echo hidden-success') `
        -WorkingDirectory $repo `
        -LogDirectory $tempRoot `
        -AsResultObject

    Assert-True ($success.ExitCode -eq 0) 'Successful command did not preserve exit code 0.'
    Assert-True ($success.Summary -match '^PASS \[successful fixture\] exit 0') 'Successful command summary was not concise.'
    Assert-True (-not $success.ShowLog) 'Successful command should keep its complete log quiet by default.'
    Assert-True ((Get-Content -LiteralPath $success.LogPath -Raw) -match 'hidden-success') 'Successful command output was not retained in its log.'

    $failure = & $runner `
        -Label 'failing fixture' `
        -Executable $env:ComSpec `
        -ArgumentList @('/d', '/c', 'echo hidden-failure & exit /b 7') `
        -WorkingDirectory $repo `
        -LogDirectory $tempRoot `
        -AsResultObject

    Assert-True ($failure.ExitCode -eq 7) 'Failing command did not preserve its nonzero exit code.'
    Assert-True ($failure.Summary -match '^FAIL \[failing fixture\] exit 7') 'Failing command summary omitted its exit code.'
    Assert-True $failure.ShowLog 'Failing command should request full log output.'
    Assert-True ((Get-Content -LiteralPath $failure.LogPath -Raw) -match 'hidden-failure') 'Failing command output was not retained in its log.'

    $failingFixture = Join-Path $tempRoot 'failing-fixture.cmd'
    Set-Content -LiteralPath $failingFixture -Encoding Ascii -Value @(
        '@echo visible-failure'
        '@exit /b 7'
    )
    $normalFailureOutput = (& 'powershell.exe' `
        -NoProfile `
        -File $runner `
        -Label 'normal failing fixture' `
        -Executable $failingFixture `
        -WorkingDirectory $repo `
        -LogDirectory $tempRoot 2>&1) -join "`n"
    $normalFailureExitCode = $LASTEXITCODE
    Assert-True ($normalFailureExitCode -eq 7) 'Normal runner entrypoint did not preserve failure exit code 7.'
    Assert-True ($normalFailureOutput -match 'FAIL \[normal failing fixture\] exit 7') 'Normal failure output omitted its concise summary.'
    Assert-True ($normalFailureOutput -match 'visible-failure') 'Normal failure output did not print the retained command log.'
}
finally {
    $resolvedTemp = [System.IO.Path]::GetFullPath($tempRoot)
    $resolvedTempParent = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
    if (-not $resolvedTemp.StartsWith($resolvedTempParent, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to clean unexpected test path: $resolvedTemp"
    }
    if (Test-Path -LiteralPath $resolvedTemp -PathType Container) {
        Remove-Item -LiteralPath $resolvedTemp -Recurse -Force
    }
}

Write-Output 'PASS run-quiet-command regression'
