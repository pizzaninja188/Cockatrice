param([string] $PowerShell = (Get-Process -Id $PID).Path)
. (Join-Path $PSScriptRoot 'workflow_test_helpers.ps1')
$fixture = New-WorkflowFixture
try {
    $arguments = @('-OracleBulk', 'oracle-cards.jsonl.gz', '-CardsXml', 'cards.xml')
    $result = Invoke-WorkflowFixture $fixture 'prepare-card-batch.ps1' $arguments (Join-Path $fixture 'nested directory')
    Assert-Workflow ($result.ExitCode -eq 0) "Preparation failed: $($result.Output)"
    $trace = @(Read-WorkflowTrace $fixture)
    Assert-Workflow ($trace.Count -eq 13) 'Expected four package target discoveries/format checks, identity, conformance, authoring lint, fingerprint and checklist steps.'
    Assert-Workflow ($trace[1].Tool -eq 'rustfmt') 'Formatting must reject before expensive preparation.'
    Assert-Workflow ($trace[8].Arguments -contains 'registry::tests::card_ids_follow_slug_convention') 'Identity check must precede conformance.'
    Assert-Workflow ($trace[9].Arguments -contains 'registry_execution_matches_reviewed_baseline') 'Conformance must precede refresh.'
    Assert-Workflow ($trace[0].Cwd -eq (Join-Path $fixture 'tricerules')) 'Wrong Cargo working directory.'
    Assert-Workflow ($trace[10].Arguments -contains 'tricerules-cards/authoring') 'Authoring-only code must be linted before review.'
    Assert-Workflow ($trace[11].Arguments -contains '--refresh-presentation') 'Preparation must use metadata-only refresh.'
    foreach ($pattern in @('rustfmt', 'card_ids_follow_slug_convention', 'registry_execution_matches_reviewed_baseline', 'cargo clippy')) {
        $before = @(Read-WorkflowTrace $fixture).Count
        [IO.File]::WriteAllText((Join-Path $fixture 'fail-pattern'), $pattern)
        $result = Invoke-WorkflowFixture $fixture 'prepare-card-batch.ps1' $arguments
        Assert-Workflow ($result.ExitCode -eq 7) "Preparation lost failing command exit status: $($result.ExitCode) $($result.Output)"
        Assert-Workflow ($result.Output -match 'fixture complete failure log') 'Preparation hid failure output.'
        $new = @(Read-WorkflowTrace $fixture | Select-Object -Skip $before)
        Assert-Workflow (@($new | Where-Object { $_.Arguments -contains '--refresh-presentation' }).Count -eq 0) 'Failed precheck reached refresh.'
    }
    Remove-Item -LiteralPath (Join-Path $fixture 'fail-pattern')
    Set-Content -LiteralPath (Join-Path $fixture 'bad-evidence') -Value 'fixture'
    $result = Invoke-WorkflowFixture $fixture 'prepare-card-batch.ps1' $arguments
    Assert-Workflow ($result.ExitCode -eq 13 -and $result.Output -match 'invalid evidence') 'Preparation did not reject invalid evidence.'
    Remove-Item -LiteralPath (Join-Path $fixture 'bad-evidence')
    $manifest = Join-Path $fixture 'focused.json'
    Set-Content -LiteralPath $manifest -Value '[{"package":"tricerules-core","target":"scenario","test":"trial::actual_card"}]'
    $result = Invoke-WorkflowFixture $fixture 'prepare-card-batch.ps1' ($arguments + @('-FocusedTests', $manifest, '-Workers', '3'))
    Assert-Workflow ($result.ExitCode -eq 0) "Focused preparation failed: $($result.Output)"
    $focused = @(Read-WorkflowTrace $fixture | Where-Object { $_.Arguments -contains 'trial::actual_card' })
    Assert-Workflow ($focused.Count -eq 1 -and $focused[0].Arguments -contains '--exact') 'Focused test did not execute exactly.'
    Assert-Workflow ($focused[0].BuildJobs -eq '3' -and $focused[0].TestThreads -eq '3') 'Explicit worker count was not honored.'
    $mapDirectory = Join-Path $fixture 'tricerules/tricerules-cards/authoring/review-maps'
    New-Item -ItemType Directory -Path $mapDirectory -Force | Out-Null
    $mapPath = Join-Path $mapDirectory 'fixture.json'
    Set-Content -LiteralPath $mapPath -Value '{}'
    $result = Invoke-WorkflowFixture $fixture 'prepare-card-batch.ps1' ($arguments + @('-ReviewMapPath', $mapPath))
    Assert-Workflow ($result.ExitCode -eq 0) "Map-selected preparation failed: $($result.Output)"
    $selectedMaps = @(Get-Content -LiteralPath (Join-Path $fixture 'selected-map-list.json') -Raw | ConvertFrom-Json)
    Assert-Workflow (@($selectedMaps).Count -eq 1 -and [string]$selectedMaps[0] -eq $mapPath) 'Map selection was not passed intact through file-backed selection.'
    Set-Content -LiteralPath (Join-Path $fixture 'zero-tests') -Value 'fixture'
    $result = Invoke-WorkflowFixture $fixture 'prepare-card-batch.ps1' $arguments
    Assert-Workflow ($result.ExitCode -ne 0 -and $result.Output -match 'Expected one executed') 'Empty test selection was accepted.'
}
finally { Remove-WorkflowFixture $fixture }
Write-Output 'PASS batch preparation order, working directory and failure boundary'
