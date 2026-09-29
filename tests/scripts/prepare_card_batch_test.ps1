param([string] $PowerShell = (Get-Process -Id $PID).Path)
. (Join-Path $PSScriptRoot 'workflow_test_helpers.ps1')
$fixture = New-WorkflowFixture
try {
    $arguments = @('-OracleBulk', 'oracle-cards.jsonl.gz', '-CardsXml', 'cards.xml')
    $result = Invoke-WorkflowFixture $fixture 'prepare-card-batch.ps1' $arguments (Join-Path $fixture 'nested directory')
    Assert-Workflow ($result.ExitCode -eq 0) "Preparation failed: $($result.Output)"
    $trace = @(Read-WorkflowTrace $fixture)
    Assert-Workflow ($trace.Count -eq 4) 'Expected identity, conformance, fingerprint and checklist steps only.'
    Assert-Workflow ($trace[0].Arguments -contains 'registry::tests::card_ids_follow_slug_convention') 'Identity check must run first.'
    Assert-Workflow ($trace[1].Arguments -contains 'registry_execution_matches_reviewed_baseline') 'Conformance must precede refresh.'
    Assert-Workflow ($trace[0].Cwd -eq (Join-Path $fixture 'tricerules')) 'Wrong Cargo working directory.'
    Assert-Workflow ($trace[2].Arguments -contains '--refresh-presentation') 'Preparation must use metadata-only refresh.'
    foreach ($call in $trace) {
        Assert-Workflow ($call.BuildJobs -eq '4' -and $call.TestThreads -eq '4') 'Four-worker cap was not inherited.'
    }
    foreach ($pattern in @('card_ids_follow_slug_convention', 'registry_execution_matches_reviewed_baseline')) {
        $before = @(Read-WorkflowTrace $fixture).Count
        [IO.File]::WriteAllText((Join-Path $fixture 'fail-pattern'), $pattern)
        $result = Invoke-WorkflowFixture $fixture 'prepare-card-batch.ps1' $arguments
        Assert-Workflow ($result.ExitCode -eq 7) "Preparation lost failing command exit status: $($result.ExitCode) $($result.Output)"
        Assert-Workflow ($result.Output -match 'fixture complete failure log') 'Preparation hid failure output.'
        $new = @(Read-WorkflowTrace $fixture | Select-Object -Skip $before)
        Assert-Workflow (@($new | Where-Object { $_.Arguments -contains '--refresh-presentation' }).Count -eq 0) 'Failed precheck reached refresh.'
    }
    Remove-Item -LiteralPath (Join-Path $fixture 'fail-pattern')
    Set-Content -LiteralPath (Join-Path $fixture 'zero-tests') -Value 'fixture'
    $result = Invoke-WorkflowFixture $fixture 'prepare-card-batch.ps1' $arguments
    Assert-Workflow ($result.ExitCode -ne 0 -and $result.Output -match 'Expected one executed') 'Empty test selection was accepted.'
}
finally { Remove-WorkflowFixture $fixture }
Write-Output 'PASS batch preparation order, working directory and failure boundary'
