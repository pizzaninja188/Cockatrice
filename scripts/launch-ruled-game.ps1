<#
.SYNOPSIS
    One command from nothing to two clients sitting in a started ruled game.

.DESCRIPTION
    Starts the tricerules sidecar, servatrice, and two Cockatrice clients, then lets each client's
    autopilot (--autopilot, see cockatrice/src/game/ruled/ruled_autopilot.cpp) do the pre-game
    ceremony: join the lobby room, create/join the game, load a deck, ready up. Both seats are
    ready within a couple of seconds of the windows appearing, so manual verification starts at
    the opening hand instead of ten clicks later.

    Processes it starts are recorded, so -Stop tears the whole set down again. Run that before
    rebuilding: a live servatrice or tricerules-server holds its .exe and the link step fails.

.PARAMETER DeckA
    Deck for the hosting seat (p1). Defaults to scripts/decks/dev-red.cod.

.PARAMETER DeckB
    Deck for the joining seat (p2). Defaults to scripts/decks/dev-blue.cod.

.PARAMETER GameName
    Game description, and the name the joining seat matches on. Change it to run two sets at once.

.PARAMETER Seed
    Fixes the engine's RNG seed, so shuffles and opening hands repeat exactly. Use it to re-run a
    board state you are debugging.

.PARAMETER Freeform
    Create a legacy freeform game instead of a ruled one.

.PARAMETER NoServers
    Skip starting the sidecar and servatrice; use the ones already running.

.PARAMETER Stop
    Stop everything a previous run started, then exit.

.EXAMPLE
    ./scripts/launch-ruled-game.ps1

.EXAMPLE
    ./scripts/launch-ruled-game.ps1 -DeckA ./scripts/decks/dev-blue.cod -Seed 12345

.EXAMPLE
    ./scripts/launch-ruled-game.ps1 -Stop
#>

[CmdletBinding()]
param(
    [string]$DeckA,
    [string]$DeckB,
    [string]$GameName,
    [int]$Seed = 0,
    [switch]$Freeform,
    [switch]$NoServers,
    [switch]$Stop
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$buildDir = Join-Path $repoRoot "build\windows-ninja-all"
$pidFile = Join-Path $buildDir "ruled-dev-game.pids"

function Get-RequiredPath {
    param([string]$Path, [string]$What, [string]$Hint)

    if (-not (Test-Path -LiteralPath $Path)) {
        throw "$What not found: $Path`n  $Hint"
    }
    return (Resolve-Path -LiteralPath $Path).Path
}

function Test-PortOpen {
    param([int]$Port)

    $tcp = New-Object System.Net.Sockets.TcpClient
    try {
        $async = $tcp.BeginConnect("127.0.0.1", $Port, $null, $null)
        if (-not $async.AsyncWaitHandle.WaitOne(200)) {
            return $false
        }
        $tcp.EndConnect($async)
        return $true
    } catch {
        return $false
    } finally {
        $tcp.Close()
    }
}

function Wait-ForPort {
    param([int]$Port, [string]$What, [int]$TimeoutSeconds = 30)

    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        if (Test-PortOpen -Port $Port) {
            return
        }
        Start-Sleep -Milliseconds 200
    }
    throw "$What never started listening on port $Port (waited ${TimeoutSeconds}s)"
}

function Register-LaunchedProcess {
    param([System.Diagnostics.Process]$Process, [string]$Label)

    "$($Process.Id) $Label" | Add-Content -LiteralPath $pidFile -Encoding utf8
    Write-Host ("  started {0} (pid {1})" -f $Label, $Process.Id) -ForegroundColor DarkGray
}

function Stop-LaunchedProcesses {
    if (-not (Test-Path -LiteralPath $pidFile)) {
        Write-Host "Nothing recorded to stop." -ForegroundColor Yellow
        return
    }

    foreach ($line in (Get-Content -LiteralPath $pidFile)) {
        if ($line -notmatch '^\s*(\d+)\s+(.*)$') {
            continue
        }
        $processId = [int]$Matches[1]
        $label = $Matches[2]
        try {
            $proc = Get-Process -Id $processId -ErrorAction Stop
            $proc.Kill()
            Write-Host ("  stopped {0} (pid {1})" -f $label, $processId) -ForegroundColor DarkGray
        } catch {
            Write-Host ("  {0} (pid {1}) was already gone" -f $label, $processId) -ForegroundColor DarkGray
        }
    }
    Remove-Item -LiteralPath $pidFile -Force
    Write-Host "Stopped." -ForegroundColor Green
}

if ($Stop) {
    Stop-LaunchedProcesses
    return
}

# A previous run's processes would fight this one over the ports.
if (Test-Path -LiteralPath $pidFile) {
    Write-Host "Cleaning up the previous run..." -ForegroundColor Yellow
    Stop-LaunchedProcesses
}

if (-not $DeckA) { $DeckA = Join-Path $PSScriptRoot "decks\dev-red.cod" }
if (-not $DeckB) { $DeckB = Join-Path $PSScriptRoot "decks\dev-blue.cod" }
if (-not $GameName) {
    if ($Freeform) { $GameName = "Freeform dev game" } else { $GameName = "Ruled dev game" }
}

$buildHint = "Build it first: ./scripts/build-ninja.ps1"
$cockatriceExe = Get-RequiredPath (Join-Path $buildDir "cockatrice\cockatrice.exe") "Cockatrice" $buildHint
$deckAPath = Get-RequiredPath $DeckA "Deck A" "Pass -DeckA <file>, or use the decks in scripts/decks/."
$deckBPath = Get-RequiredPath $DeckB "Deck B" "Pass -DeckB <file>, or use the decks in scripts/decks/."

$rulesPort = 17381
if ($env:TRICERULES_PORT) { $rulesPort = [int]$env:TRICERULES_PORT }
$serverPort = 4747

if (-not $NoServers) {
    $servatriceExe = Get-RequiredPath (Join-Path $buildDir "servatrice\servatrice.exe") "Servatrice" $buildHint
    $sidecarExe = Get-RequiredPath (Join-Path $repoRoot "tricerules\target\release\tricerules-server.exe") `
        "tricerules-server" "Build it first: cargo build --release --manifest-path ./tricerules/Cargo.toml -p tricerules-server"
    $configPath = Get-RequiredPath (Join-Path $repoRoot "servatrice-local.ini") "servatrice-local.ini" "Expected in the repo root."

    Write-Host "Starting servers..." -ForegroundColor Cyan

    if (Test-PortOpen -Port $rulesPort) {
        Write-Host "  tricerules-server already listening on $rulesPort - reusing it" -ForegroundColor DarkGray
    } else {
        $sidecar = Start-Process -FilePath $sidecarExe -PassThru -WorkingDirectory $repoRoot
        Register-LaunchedProcess -Process $sidecar -Label "tricerules-server"
        Wait-ForPort -Port $rulesPort -What "tricerules-server"
    }

    if (Test-PortOpen -Port $serverPort) {
        throw "Something is already listening on port $serverPort. Run this script with -Stop, or close the other servatrice."
    }

    # Read by startRuledSidecarSession; leaving it unset means a fresh shuffle every run.
    if ($Seed -ne 0) {
        $env:COCKATRICE_RULED_SEED = "$Seed"
        Write-Host "  engine seed pinned to $Seed" -ForegroundColor DarkGray
    } else {
        Remove-Item Env:\COCKATRICE_RULED_SEED -ErrorAction SilentlyContinue
    }

    $servatrice = Start-Process -FilePath $servatriceExe -PassThru -WorkingDirectory $repoRoot `
        -ArgumentList "--config", "`"$configPath`""
    Register-LaunchedProcess -Process $servatrice -Label "servatrice"
    Wait-ForPort -Port $serverPort -What "servatrice"
} else {
    Write-Host "Using the servers already running." -ForegroundColor Cyan
    Wait-ForPort -Port $serverPort -What "servatrice" -TimeoutSeconds 5
}

# Inherited by both clients: the autopilot reads these directly, keeping main.cpp's option list to
# the two that actually vary per seat.
$env:COCKATRICE_AUTOPILOT_GAME = $GameName
if ($Freeform) { $env:COCKATRICE_AUTOPILOT_RULED = "0" } else { $env:COCKATRICE_AUTOPILOT_RULED = "1" }

Write-Host "Starting clients..." -ForegroundColor Cyan

$hostClient = Start-Process -FilePath $cockatriceExe -PassThru -ArgumentList `
    "-c", "p1:pass@127.0.0.1:$serverPort", "--autopilot", "host", "--autopilot-deck", "`"$deckAPath`""
Register-LaunchedProcess -Process $hostClient -Label "cockatrice p1 (host)"

# The joining seat retries on every game-list update, so it is allowed to lose this race; the
# stagger just keeps the log readable.
Start-Sleep -Milliseconds 750

$joinClient = Start-Process -FilePath $cockatriceExe -PassThru -ArgumentList `
    "-c", "p2:pass@127.0.0.1:$serverPort", "--autopilot", "join", "--autopilot-deck", "`"$deckBPath`""
Register-LaunchedProcess -Process $joinClient -Label "cockatrice p2 (join)"

Write-Host ""
if ($Freeform) {
    Write-Host "Freeform game '$GameName' coming up." -ForegroundColor Green
} else {
    Write-Host "Ruled game '$GameName' coming up." -ForegroundColor Green
}
Write-Host "  p1 $([System.IO.Path]::GetFileName($deckAPath))   p2 $([System.IO.Path]::GetFileName($deckBPath))"
Write-Host "  Both seats ready themselves; the game starts on its own."
Write-Host "  If a seat stays in the lobby, check its log for 'ruled_autopilot'."
Write-Host ""
Write-Host "  Tear down:  ./scripts/launch-ruled-game.ps1 -Stop" -ForegroundColor DarkGray
