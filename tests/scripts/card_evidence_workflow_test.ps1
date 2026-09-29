param([string] $PowerShell = (Get-Process -Id $PID).Path)
. (Join-Path $PSScriptRoot 'workflow_test_helpers.ps1')
$fixture = New-WorkflowFixture
try {
    foreach ($name in @('check-card-evidence.ps1', 'card-evidence.psm1')) {
        Copy-Item -LiteralPath (Join-Path $sourceRepo "scripts/$name") -Destination (Join-Path $fixture "scripts/$name")
    }
    $maps = Join-Path $fixture 'tricerules/tricerules-cards/authoring/review-maps'
    New-Item -ItemType Directory -Path $maps -Force | Out-Null
    @{ semantic_fixtures = @(@{ test = 'alpha::case' }, @{ test = 'beta::case' }, @{ test = 'scenario module::case' }) } |
        ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $maps 'fixture.json')
    # Exercise orchestration separately from native process execution (covered by the real gate).
    Set-Content -LiteralPath (Join-Path $fixture 'scripts/run-quiet-command.ps1') -Value @'
param($Label, $Executable, $ArgumentList, $WorkingDirectory, $LogDirectory, [switch] $AsResultObject)
$root = Split-Path -Parent $PSScriptRoot
@{ Tool = $Executable; Arguments = $ArgumentList } | ConvertTo-Json -Compress | Add-Content (Join-Path $root 'trace.jsonl')
$lines = @()
$code = 0
if ($Executable -eq 'cargo') {
    if ($ArgumentList -notcontains '--no-run' -or $ArgumentList -notcontains '--message-format=json') { throw 'Expected one artifact build' }
    for ($i = 0; $i -lt $ArgumentList.Count; $i++) {
        if ($ArgumentList[$i] -ne '--test') { continue }
        $name = $ArgumentList[$i + 1]
        $lines += @{ reason = 'compiler-artifact'; target = @{ name = $name; kind = @('test') };
            profile = @{ test = $true }; executable = "C:/fixture binaries/$name.exe" } | ConvertTo-Json -Compress
    }
} elseif ($Executable -like 'C:/fixture binaries/*') {
    if (Test-Path (Join-Path $root 'binary-fails')) { $code = 19; $lines = @('binary listing failed') }
    elseif ($ArgumentList -notcontains '--ignored' -or (Test-Path (Join-Path $root 'ignored'))) {
        $lines = if ($Executable -like '*scenario.exe') { @('module::case: test') } else { @('case: test') }
    }
}
$log = Join-Path $LogDirectory ([guid]::NewGuid().ToString() + '.log')
[IO.File]::WriteAllLines($log, [string[]] $lines)
[pscustomobject]@{ Summary = "fixture $Label"; ExitCode = $code; ShowLog = ($code -ne 0); LogPath = $log }
'@
    $result = Invoke-WorkflowFixture $fixture 'check-card-evidence.ps1'
    Assert-Workflow ($result.ExitCode -eq 0) "Evidence orchestration failed: $($result.Output)"
    $trace = @(Read-WorkflowTrace $fixture)
    Assert-Workflow (@($trace | Where-Object Tool -eq 'cargo').Count -eq 2) 'Expected two Cargo builds for three targets.'
    Assert-Workflow (@($trace | Where-Object Tool -like 'C:/fixture binaries/*').Count -eq 6) 'Missing normal or ignored binary listing.'
    foreach ($failure in @('ignored', 'binary-fails')) {
        Set-Content -LiteralPath (Join-Path $fixture $failure) -Value 'fixture'
        $result = Invoke-WorkflowFixture $fixture 'check-card-evidence.ps1'
        Assert-Workflow ($result.ExitCode -ne 0) "Accepted $failure evidence"
        Remove-Item -LiteralPath (Join-Path $fixture $failure)
    }
}
finally { Remove-WorkflowFixture $fixture }
Write-Output 'PASS evidence builds each package once and rejects ignored or failed binary listings'
