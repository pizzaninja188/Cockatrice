param([string] $PowerShell = (Get-Process -Id $PID).Path)
. (Join-Path $PSScriptRoot 'workflow_test_helpers.ps1')
$fixture = New-WorkflowFixture
try {
    $data = Join-Path $fixture 'tricerules/tricerules-cards/data'
    $maps = Join-Path $fixture 'tricerules/tricerules-cards/authoring/review-maps'
    $scenarios = Join-Path $fixture 'tricerules/tricerules-core/tests/scenario'
    foreach ($dir in @($data, $maps, $scenarios)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
    [IO.File]::WriteAllText((Join-Path $data 'other.ron'), 'existing card')
    [IO.File]::WriteAllText((Join-Path $scenarios 'example.rs'), 'existing scenario')
    [IO.File]::WriteAllText((Join-Path $fixture 'inventory.txt'), "scripts/verify.ps1`ntricerules/Cargo.toml`n")
    $tool = Join-Path $fixture 'bin/tool.ps1'
    $toolText = [IO.File]::ReadAllText($tool)
    $marker = '$record = @{ Tool = $tool;'
    $injection = @'
if ($tool -eq 'git' -and $arguments -contains '--stage') { exit 0 }
if ($tool -eq 'git' -and $arguments -contains 'ls-files') {
    $names = @(Get-Content -LiteralPath (Join-Path $root 'inventory.txt'))
    foreach ($directory in @('tricerules/tricerules-cards/data', 'tricerules/tricerules-cards/authoring/review-maps', 'tricerules/tricerules-core/tests')) {
        $names += @(Get-ChildItem -LiteralPath (Join-Path $root $directory) -File -Recurse -Force | ForEach-Object {
            $_.FullName.Substring($root.Length + 1).Replace('\', '/')
        })
    }
    [Console]::Out.Write([string]::Join("`0", $names) + "`0")
    exit 0
}
if ($tool -eq 'cargo' -and $arguments -contains '--version') { Write-Output 'fixture cargo 1'; exit 0 }
if ($tool -eq 'cargo' -and $arguments -contains '--exact' -and (Test-Path (Join-Path $root 'ignored-test'))) {
    Write-Output 'test result: ok. 0 passed; 0 failed; 1 ignored;'
    exit 0
}
if ($tool -eq 'cargo' -and $arguments -contains 'clippy' -and (Test-Path (Join-Path $root 'mutate-input'))) {
    [IO.File]::AppendAllText((Join-Path $root 'tricerules/tricerules-core/tests/scenario/example.rs'), 'changed during verification')
}
'@
    if (-not $toolText.Contains($marker)) { throw 'Fixture command marker is missing.' }
    [IO.File]::WriteAllText($tool, $toolText.Replace($marker, $injection + "`n" + $marker))

    $record = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Rust', '-CardData', '-RecordRoutineBaseline')
    Assert-Workflow ($record.ExitCode -eq 0) "Full baseline recording failed: $($record.Output)"
    $summaries = @(Get-ChildItem -LiteralPath (Join-Path $fixture 'build/verification-logs') -Filter summary.json -Recurse)
    Assert-Workflow ($summaries.Count -eq 1) 'The recorded full run must create one summary.'
    $baselinePath = $summaries[0].FullName
    $baseline = Get-Content -LiteralPath $baselinePath -Raw | ConvertFrom-Json
    Assert-Workflow ($baseline.Status -eq 'Pass' -and $baseline.RoutineBaseline.StartSha256 -eq $baseline.RoutineBaseline.EndSha256) 'Full baseline was not bound to stable inputs.'

    [IO.File]::WriteAllText((Join-Path $data 'example.ron'), 'new card')
    @{ tokens = @(); semantic_fixtures = @(@{ test = 'scenario example::actual_card'; covers = 'actual behavior' });
        complete_definition_review_confirmed = $true } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $maps 'example.json')
    [IO.File]::AppendAllText((Join-Path $scenarios 'example.rs'), ' plus actual card scenario')
    $before = @(Read-WorkflowTrace $fixture).Count
    $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Rust', '-CardData', '-RoutineRonBaseline', $baselinePath)
    Assert-Workflow ($result.ExitCode -eq 0) "Eligible narrow gate failed: $($result.Output)"
    $trace = @(Read-WorkflowTrace $fixture | Select-Object -Skip $before)
    Assert-Workflow (@($trace | Where-Object { $_.Tool -eq 'cargo' -and $_.Arguments -contains 'tricerules-cards' -and $_.Arguments[0] -eq 'test' }).Count -eq 1) 'Full card package tests were not run.'
    Assert-Workflow (@($trace | Where-Object { $_.Tool -eq 'cargo' -and $_.Arguments -contains 'conformance' }).Count -eq 1) 'Full conformance target was not run.'
    Assert-Workflow (@($trace | Where-Object { $_.Arguments -contains 'example::actual_card' -and $_.Arguments -contains '--exact' }).Count -eq 1) 'Affected scenario was not executed exactly.'
    Assert-Workflow (@($trace | Where-Object { $_.Tool -eq 'cargo' -and $_.Arguments.Count -eq 1 -and $_.Arguments[0] -eq 'test' }).Count -eq 0) 'Narrow gate unexpectedly ran the entire workspace test suite.'

    Set-Content -LiteralPath (Join-Path $fixture 'zero-tests') -Value 'fixture'
    $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Rust', '-CardData', '-RoutineRonBaseline', $baselinePath)
    Assert-Workflow ($result.ExitCode -ne 0 -and $result.Output -match 'Expected exactly one executed') 'A zero-test semantic result was accepted.'
    Remove-Item -LiteralPath (Join-Path $fixture 'zero-tests')
    Set-Content -LiteralPath (Join-Path $fixture 'ignored-test') -Value 'fixture'
    $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Rust', '-CardData', '-RoutineRonBaseline', $baselinePath)
    Assert-Workflow ($result.ExitCode -ne 0 -and $result.Output -match 'Expected exactly one executed') 'An ignored semantic test was accepted.'
    Remove-Item -LiteralPath (Join-Path $fixture 'ignored-test')
    Set-Content -LiteralPath (Join-Path $fixture 'fail-pattern') -Value 'cargo test.*example::actual_card' -NoNewline
    $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Rust', '-CardData', '-RoutineRonBaseline', $baselinePath)
    Assert-Workflow ($result.ExitCode -eq 7 -and $result.Output -match 'fixture complete failure log') 'A failing semantic scenario lost its exit code or log.'
    Remove-Item -LiteralPath (Join-Path $fixture 'fail-pattern')

    $scenarioBytes = [IO.File]::ReadAllBytes((Join-Path $scenarios 'example.rs'))
    Set-Content -LiteralPath (Join-Path $fixture 'mutate-input') -Value 'fixture'
    $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Rust', '-CardData', '-RoutineRonBaseline', $baselinePath)
    Assert-Workflow ($result.ExitCode -ne 0 -and $result.Output -match 'changed during the run') 'Accepted inputs changed while verification was running.'
    [IO.File]::WriteAllBytes((Join-Path $scenarios 'example.rs'), $scenarioBytes)
    Remove-Item -LiteralPath (Join-Path $fixture 'mutate-input')

    $saved = [IO.File]::ReadAllText($baselinePath)
    $baseline.Status = 'Fail'
    $failed = $baseline | ConvertTo-Json -Depth 8
    $baseline.Status = 'Pass'
    $baseline.Steps[0].Status = 'NotRun'
    $missingStep = $baseline | ConvertTo-Json -Depth 8
    foreach ($invalid in @('{"Status":"Pass"}', $failed, $missingStep)) {
        [IO.File]::WriteAllText($baselinePath, $invalid)
        $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Rust', '-CardData', '-RoutineRonBaseline', $baselinePath)
        Assert-Workflow ($result.ExitCode -ne 0) 'Accepted an incomplete or failed baseline summary.'
    }
    [IO.File]::WriteAllText($baselinePath, $saved)
    $snapshotPath = Join-Path (Split-Path -Parent $baselinePath) 'routine-inputs.json'
    [IO.File]::AppendAllText($snapshotPath, ' ')
    $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Rust', '-CardData', '-RoutineRonBaseline', $baselinePath)
    Assert-Workflow ($result.ExitCode -ne 0 -and $result.Output -match 'snapshot is missing or changed') 'Accepted a changed baseline snapshot.'
}
finally { Remove-WorkflowFixture $fixture }
Write-Output 'PASS routine RON verification workflow'
