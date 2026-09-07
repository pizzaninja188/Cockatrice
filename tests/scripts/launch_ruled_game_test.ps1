$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$fixture = Join-Path $repo ('build/launcher-test-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path "$fixture/scripts" -Force | Out-Null
Copy-Item "$repo/scripts/launch-ruled-game.ps1" "$fixture/scripts/launch-ruled-game.ps1"
$shell = (Get-Process -Id $PID).Path
try {
    foreach ($buildExit in @(0, 7)) {
        @'
$helper = Start-Process powershell.exe -WindowStyle Hidden -PassThru -ArgumentList @('-NoProfile', '-Command', 'Start-Sleep -Seconds 60')
$helper.Id | Set-Content "$PSScriptRoot/helper.pid"
'@ + "`nexit $buildExit" | Set-Content "$fixture/scripts/build-ninja.ps1"
        $launcher = Start-Process $shell -WindowStyle Hidden -PassThru -ArgumentList @(
            '-NoProfile', '-File', "`"$fixture/scripts/launch-ruled-game.ps1`"", '-Dev'
        ) -RedirectStandardOutput "$fixture/stdout.log" -RedirectStandardError "$fixture/stderr.log"
        try {
            if (-not $launcher.WaitForExit(15000)) {
                throw 'Launcher waited for the surviving build helper after the build finished.'
            }
            $errorText = Get-Content "$fixture/stderr.log" -Raw
            if ($buildExit -eq 0 -and $errorText -notmatch 'Cockatrice not found') {
                throw "Successful build did not advance to artifact validation: $errorText"
            }
            if ($buildExit -eq 7 -and $errorText -notmatch 'Build failed with exit code 7') {
                throw "Build failure did not preserve exit code 7: $errorText"
            }
            $helperId = [int](Get-Content "$fixture/scripts/helper.pid")
            if (-not (Get-Process -Id $helperId -ErrorAction SilentlyContinue)) {
                throw 'Fixture helper exited before launcher verification.'
            }
        } finally {
            if (-not $launcher.HasExited) { $launcher.Kill(); $launcher.WaitForExit() }
            if (Test-Path "$fixture/scripts/helper.pid") {
                Stop-Process -Id ([int](Get-Content "$fixture/scripts/helper.pid")) -ErrorAction SilentlyContinue
            }
        }
    }
    Write-Host 'PASS: launcher waits only for the build and preserves build failures.'
} finally {
    # The unique fixture is created directly beneath this checkout's build directory.
    $resolved = [IO.Path]::GetFullPath($fixture)
    if (-not $resolved.StartsWith([IO.Path]::GetFullPath((Join-Path $repo 'build')) + [IO.Path]::DirectorySeparatorChar)) {
        throw "Unexpected fixture path: $resolved"
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
