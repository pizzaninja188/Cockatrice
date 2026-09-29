param([string] $PowerShell = (Get-Process -Id $PID).Path)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$directory = Join-Path $repo ('build/authoring-loop-test-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $directory | Out-Null
$draft = Join-Path $directory 'draft_trial.ron'
$manifest = Join-Path $directory 'batch.json'
$ron = '(id:"draft_trial",name:"Draft Trial",face_id:"draft_trial",mana_cost:"{2}{U}",types:["Sorcery"],spell_effect:[Draw(count:2)])'
[IO.File]::WriteAllText($draft,$ron)
$row = @{ family='draw'; card='draft_trial'; mana=@(0,1,0,0,0,2); surface='spell'; recipient=0; count=2; food=0 }
function Invoke-Draft([bool] $ExpectedSuccess, [string] $FailurePattern = 'exact hand') {
    $result = & (Join-Path $repo 'scripts/run-quiet-command.ps1') -Label 'Draft loop integration' `
        -Executable $PowerShell -ArgumentList @('-NoProfile','-File',(Join-Path $repo 'scripts/test-card-drafts.ps1'),'-BatchPath',$manifest) -AsResultObject
    if (($result.ExitCode -eq 0) -ne $ExpectedSuccess) {
        Get-Content -LiteralPath $result.LogPath | Out-Host
        throw "Unexpected draft result: $($result.ExitCode)"
    }
    Write-Host $result.Summary
    if (-not $ExpectedSuccess -and -not (Select-String -LiteralPath $result.LogPath -Pattern $FailurePattern)) { throw 'Draft failure did not prove the intended changed semantics.' }
}
# Write UTF8 without BOM for serde_json under both PowerShell versions.
function Write-Batch {
    $json = @{ drafts=@('draft_trial.ron'); rows=@($row) } | ConvertTo-Json -Depth 6
    [IO.File]::WriteAllText($manifest,$json,[Text.UTF8Encoding]::new($false))
}
Write-Batch
Invoke-Draft $true
Import-Module (Join-Path $repo 'scripts/card-evidence.psm1') -Force
$artifact = & (Join-Path $repo 'scripts/run-quiet-command.ps1') -Label 'Draft runner artifact' `
    -WorkingDirectory (Join-Path $repo 'tricerules') -Executable cargo `
    -ArgumentList @('test','--quiet','-p','tricerules-core','--features','authoring','--test','scenario','--no-run','--message-format=json') -AsResultObject
if ($artifact.ExitCode -ne 0) { Get-Content $artifact.LogPath | Out-Host; exit $artifact.ExitCode }
$binary = (Get-CardEvidenceExecutables @(Get-Content $artifact.LogPath) @('scenario'))['scenario']
$hash = (Get-FileHash -LiteralPath $binary).Hash
$stamp = (Get-Item -LiteralPath $binary).LastWriteTimeUtc
[IO.File]::WriteAllText($draft,$ron.Replace('count:2','count:3'))
Invoke-Draft $false
$row.count = 3
Write-Batch
Invoke-Draft $true
if ((Get-FileHash -LiteralPath $binary).Hash -ne $hash -or (Get-Item -LiteralPath $binary).LastWriteTimeUtc -ne $stamp) { throw 'Draft-only edits rebuilt the test binary.' }
# Exercise a new family against fresh external mechanics, not only shipped calibration cards.
$ron = '(id:"draft_trial",name:"Draft Trial",face_id:"draft_trial",types:["Artifact"],activated_abilities:[(ability_id:"activated_01",presentation:Fallback,costs:[Tap],effect:[ProduceMana(options:[(c:2)])])])'
[IO.File]::WriteAllText($draft,$ron)
$row = @{ family='mana_activation'; card='draft_trial'; ability_index=0; produced=@(0,0,0,0,0,2) }
Write-Batch
Invoke-Draft $true
[IO.File]::WriteAllText($draft,$ron.Replace('c:2','c:3'))
Invoke-Draft $false 'exact mana'
$row.produced = @(0,0,0,0,0,3)
Write-Batch
Invoke-Draft $true
if ((Get-FileHash -LiteralPath $binary).Hash -ne $hash -or (Get-Item -LiteralPath $binary).LastWriteTimeUtc -ne $stamp) { throw 'New-family draft edits rebuilt the test binary.' }
Write-Output "PASS draft edits change real engine behavior without rebuilding; evidence retained at $directory"
