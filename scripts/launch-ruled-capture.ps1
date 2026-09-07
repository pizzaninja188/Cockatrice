<# Local, isolated live reconstruction. Called by launch-ruled-game.ps1 -Capture. #>
[CmdletBinding()]
param(
    [string]$Capture,
    [Nullable[uint64]]$StopAfter,
    [switch]$AllowBuildMismatch,
    [string]$GameName,
    [switch]$Trace,
    [switch]$Stop,
    [string]$RunDirectory
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'ruled-capture-common.ps1')
$taskRepo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$taskRunsRoot = [IO.Path]::GetFullPath((Join-Path $taskRepo 'build\ruled-resume'))

function Stop-CapturedRun([string]$Directory) {
    $taskRun = [IO.Path]::GetFullPath($Directory).TrimEnd('\','/')
    if ([IO.Path]::GetDirectoryName($taskRun) -ne $taskRunsRoot -or
        [IO.Path]::GetFileName($taskRun) -notmatch '^[0-9a-f]{32}$') { throw 'RunDirectory must identify a run under build/ruled-resume.' }
    if ((Get-Item -LiteralPath $taskRun).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'RunDirectory cannot be a link.' }
    $taskRecord = Join-Path $taskRun 'processes.json'
    if (-not (Test-Path -LiteralPath $taskRecord)) { throw 'Run process record is missing.' }
    foreach ($taskEntry in @((Get-Content -Raw -LiteralPath $taskRecord | ConvertFrom-Json))) {
        $taskProcess = Get-Process -Id $taskEntry.id -ErrorAction SilentlyContinue
        if (-not $taskProcess) { continue }
        if ($taskProcess.Path -ne $taskEntry.executable -or
            $taskProcess.StartTime.ToUniversalTime().Ticks.ToString() -ne $taskEntry.started_ticks) {
            throw "Process $($taskEntry.id) no longer matches this run; it was not stopped."
        }
        Stop-Process -Id $taskProcess.Id -ErrorAction Stop
        $taskProcess.WaitForExit(10000) | Out-Null
    }
    Write-Host "Stopped captured run. Evidence retained at $taskRun"
}
if ($Stop) {
    if (-not $RunDirectory -or $Capture) { throw '-Stop requires -RunDirectory, without -Capture.' }
    Stop-CapturedRun $RunDirectory
    return
}
if ($RunDirectory -or -not $Capture) { throw 'Start with -Capture; -RunDirectory is only for stopping an existing run.' }

# A full build checks all consumers before any server or client starts. Existing unrelated runs
# are never killed or reused, including on a locked-executable build failure.
& (Join-Path $PSScriptRoot 'run-quiet-command.ps1') -Label 'Captured game launch build' -Executable powershell.exe `
    -ArgumentList @('-NoProfile','-File',(Join-Path $PSScriptRoot 'build-ninja.ps1'))
if ($LASTEXITCODE -ne 0) { throw "Build failed ($LASTEXITCODE); no processes launched." }
$taskTool = Get-RuledCaptureTool
$taskInput = Open-RuledCaptureInput -Capture $Capture -Tool $taskTool
$taskRun = Join-Path $taskRunsRoot ([guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $taskRun | Out-Null
$taskPlanDir = Join-Path $taskRun 'reconstruction'
try {
    $taskReplay = Join-Path $taskRepo 'tricerules\target\release\tricerules-replay.exe'
    $taskArgs = @('--capture',$taskInput.Directory,'--resume-plan','--output',$taskPlanDir)
    if ($null -ne $StopAfter) { $taskArgs += @('--stop-after', $StopAfter.ToString()) }
    if ($AllowBuildMismatch) { $taskArgs += '--allow-build-mismatch' }
    & $taskReplay @taskArgs | Out-Host
    if ($LASTEXITCODE -ne 0 -and -not ($AllowBuildMismatch -and $LASTEXITCODE -eq 2)) { throw 'Capture reconstruction failed; no processes launched.' }
} finally { Close-RuledCaptureInput $taskInput.Temporary }
$taskPlan = Get-Content -Raw -LiteralPath (Join-Path $taskPlanDir 'resume-plan.json') | ConvertFrom-Json
$taskPlayers = @($taskPlan.session_start.player_ids)
if ($taskPlayers.Count -lt 2 -or $taskPlayers.Count -gt 99) { throw 'Launcher supports 2 through 99 seats.' }
if (-not $GameName) { $GameName = 'Resumed ' + $taskPlan.parent_capture_id }

function Get-FreeLoopbackPort {
    $taskListener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    try { $taskListener.Start(); return $taskListener.LocalEndpoint.Port } finally { $taskListener.Stop() }
}
function Wait-CapturePort([int]$Port, [System.Diagnostics.Process]$Process) {
    $taskDeadline = [DateTime]::UtcNow.AddSeconds(30)
    while ([DateTime]::UtcNow -lt $taskDeadline) {
        if ($Process.HasExited) { throw "Process $($Process.Id) exited before opening port $Port." }
        $taskSocket = [Net.Sockets.TcpClient]::new()
        try { $taskSocket.Connect('127.0.0.1',$Port); return } catch { Start-Sleep -Milliseconds 200 } finally { $taskSocket.Dispose() }
    }
    throw "Timed out waiting for port $Port."
}
$taskRulesPort = Get-FreeLoopbackPort
do { $taskServerPort = Get-FreeLoopbackPort } while ($taskServerPort -eq $taskRulesPort)
$taskConfig = @"
[server]
name=Local captured game
host=127.0.0.1
port=$taskServerPort
number_pools=1
websocket_number_pools=0
writelog=1
logfile=servatrice.log
idleclienttimeout=0
[authentication]
method=none
[users]
minnamelength=1
maxnamelength=20
[database]
type=none
[rooms]
method=config
roomlist\size=1
roomlist\1\name=Captured game
roomlist\1\autojoin=true
[game]
max_game_inactivity_time=0
[security]
max_users_per_address=100
max_command_count_per_interval=10000
"@
$taskConfigPath = Join-Path $taskRun 'servatrice.ini'
[IO.File]::WriteAllText($taskConfigPath, $taskConfig)
$taskProcesses = [Collections.Generic.List[object]]::new()
function Save-CaptureProcess([System.Diagnostics.Process]$Process, [string]$Executable) {
    $taskProcesses.Add(@{id=$Process.Id; executable=[IO.Path]::GetFullPath($Executable); started_ticks=$Process.StartTime.ToUniversalTime().Ticks.ToString()})
    [IO.File]::WriteAllText((Join-Path $taskRun 'processes.json'), (ConvertTo-Json -InputObject @($taskProcesses.ToArray())))
}
$taskEnvNames = @('TRICERULES_PORT','TRICERULES_DEV_COMMANDS','COCKATRICE_RULED_DEV','COCKATRICE_RULED_SEED',
    'COCKATRICE_RULED_CAPTURE_DIR','COCKATRICE_RULED_CAPTURE','COCKATRICE_RULED_RESUME_PLAN','COCKATRICE_RULED_RESUME_POLICY',
    'COCKATRICE_AUTOPILOT_PLAYERS','COCKATRICE_AUTOPILOT_GAME','COCKATRICE_AUTOPILOT_HOST_USER','COCKATRICE_AUTOPILOT_RULED','COCKATRICE_RULED_DEBUG')
$taskSavedEnv = @{}
foreach ($taskName in $taskEnvNames) { $taskSavedEnv[$taskName] = [Environment]::GetEnvironmentVariable($taskName,'Process') }
try {
    $env:TRICERULES_PORT = $taskRulesPort.ToString()
    $env:TRICERULES_DEV_COMMANDS = if ($taskPlan.effective_dev_commands_enabled) { '1' } else { '0' }
    $env:COCKATRICE_RULED_DEV = $env:TRICERULES_DEV_COMMANDS
    $env:COCKATRICE_RULED_SEED = $null
    $env:COCKATRICE_RULED_CAPTURE_DIR = Join-Path $taskRun 'captures'
    $env:COCKATRICE_RULED_CAPTURE = '1'
    $env:COCKATRICE_RULED_RESUME_POLICY = $null
    $env:COCKATRICE_RULED_RESUME_PLAN = Join-Path $taskPlanDir 'resume-plan.pb'
    $env:COCKATRICE_RULED_DEBUG = if ($Trace) { '1' } else { '0' }
    $taskSidecar = Start-Process -FilePath (Join-Path $taskRepo 'tricerules\target\release\tricerules-server.exe') `
        -WindowStyle Hidden -PassThru -WorkingDirectory $taskRun -RedirectStandardError (Join-Path $taskRun 'sidecar.log')
    Save-CaptureProcess $taskSidecar (Join-Path $taskRepo 'tricerules\target\release\tricerules-server.exe')
    Wait-CapturePort $taskRulesPort $taskSidecar
    $taskServer = Start-Process -FilePath (Join-Path $taskRepo 'build\windows-ninja-all\servatrice\servatrice.exe') `
        -WindowStyle Hidden -PassThru -WorkingDirectory $taskRun -ArgumentList @('--config',('"' + $taskConfigPath + '"'))
    Save-CaptureProcess $taskServer (Join-Path $taskRepo 'build\windows-ninja-all\servatrice\servatrice.exe')
    Wait-CapturePort $taskServerPort $taskServer
    # Client environments contain only their toolbar policy, never the maintainer plan path.
    $env:COCKATRICE_RULED_RESUME_PLAN = $null
    $env:COCKATRICE_AUTOPILOT_PLAYERS = $taskPlayers.Count.ToString()
    $env:COCKATRICE_AUTOPILOT_GAME = $GameName
    $env:COCKATRICE_AUTOPILOT_HOST_USER = 'resume0'
    $env:COCKATRICE_AUTOPILOT_RULED = '1'
    for ($taskSeat = 0; $taskSeat -lt $taskPlayers.Count; ++$taskSeat) {
        $taskPlayer = $taskPlayers[$taskSeat]
        $taskSeatDir = Join-Path $taskRun "seat-$taskPlayer"
        New-Item -ItemType Directory -Path $taskSeatDir | Out-Null
        $taskDeck = @($taskPlan.display_decks | Where-Object { $_.player_id -eq $taskPlayer })
        if ($taskDeck.Count -ne 1) { throw "Missing display deck for seat $taskPlayer." }
        $taskDeckPath = Join-Path $taskSeatDir 'display.cod'
        $taskXmlSettings = [Xml.XmlWriterSettings]::new()
        $taskXmlSettings.Indent = $true
        $taskXmlSettings.Encoding = [Text.UTF8Encoding]::new($false)
        $taskXml = [Xml.XmlWriter]::Create($taskDeckPath, $taskXmlSettings)
        try {
            $taskXml.WriteStartElement('cockatrice_deck'); $taskXml.WriteAttributeString('version','1')
            $taskXml.WriteElementString('deckname',"Captured seat $taskPlayer")
            $taskXml.WriteStartElement('zone'); $taskXml.WriteAttributeString('name','main')
            foreach ($taskCardName in $taskDeck[0].mainboard_card_name) {
                $taskXml.WriteStartElement('card'); $taskXml.WriteAttributeString('number','1'); $taskXml.WriteAttributeString('name',$taskCardName); $taskXml.WriteEndElement()
            }
            $taskXml.WriteEndElement(); $taskXml.WriteEndElement()
        } finally { $taskXml.Dispose() }
        $taskPolicy = @($taskPlan.restored_auto_pass_policies | Where-Object { $_.player_id -eq $taskPlayer })
        if ($taskPolicy.Count -ne 1) { throw "Missing toolbar policy for seat $taskPlayer." }
        $env:COCKATRICE_RULED_RESUME_POLICY = Join-Path $taskSeatDir 'policy.json'
        [IO.File]::WriteAllText($env:COCKATRICE_RULED_RESUME_POLICY, ($taskPolicy[0] | ConvertTo-Json -Depth 10))
        $taskRole = if ($taskSeat -eq 0) { 'host' } else { 'join' }
        $taskClientArgs = @('-c',"resume${taskSeat}:pass@127.0.0.1:$taskServerPort",'--autopilot',$taskRole,'--autopilot-deck',('"' + $taskDeckPath + '"'),'--debug-output')
        if ($taskPlan.effective_dev_commands_enabled) { $taskClientArgs += '--dev-console' }
        $taskClient = Start-Process -FilePath (Join-Path $taskRepo 'build\windows-ninja-all\cockatrice\cockatrice.exe') `
            -PassThru -WorkingDirectory $taskSeatDir -ArgumentList $taskClientArgs
        Save-CaptureProcess $taskClient (Join-Path $taskRepo 'build\windows-ninja-all\cockatrice\cockatrice.exe')
        $taskDeadline = [DateTime]::UtcNow.AddSeconds(60)
        $taskJoined = $false
        while ([DateTime]::UtcNow -lt $taskDeadline) {
            if ($taskClient.HasExited) { throw "Client for seat $taskPlayer exited." }
            $taskLog = Join-Path $taskSeatDir 'qdebug.txt'
            if ((Test-Path -LiteralPath $taskLog) -and (Select-String -LiteralPath $taskLog -Pattern "joined game .* as player $taskPlayer(?:\s|$)" -Quiet)) { $taskJoined = $true; break }
            Start-Sleep -Milliseconds 200
        }
        if (-not $taskJoined) { throw "Client did not join as recorded seat $taskPlayer; inspect $taskSeatDir." }
    }
    Write-Host "Resumed clients launched at accepted command $($taskPlan.stop_after). Evidence: $taskRun"
    Write-Host "Stop: ./scripts/launch-ruled-game.ps1 -Stop -RunDirectory '$taskRun'"
} catch {
    if ($taskProcesses.Count) { Stop-CapturedRun $taskRun }
    throw
} finally {
    foreach ($taskName in $taskEnvNames) { [Environment]::SetEnvironmentVariable($taskName,$taskSavedEnv[$taskName],'Process') }
}
