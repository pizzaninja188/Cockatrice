$ErrorActionPreference = "Stop"
$p = Join-Path $PSScriptRoot "..\build\windows-msvc-all\cockatrice\Release\cockatrice.exe" | Resolve-Path
if (-not (Test-Path -LiteralPath $p)) {
    throw "Cockatrice not found: $p"
}
Start-Process $p -ArgumentList "-c", "p1:pass@127.0.0.1:4747"
Start-Process $p -ArgumentList "-c", "p2:pass@127.0.0.1:4747"
