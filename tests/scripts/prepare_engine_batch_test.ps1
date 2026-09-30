param([string] $PowerShell = (Get-Process -Id $PID).Path)
. (Join-Path $PSScriptRoot 'workflow_test_helpers.ps1')
$fixture = New-WorkflowFixture
try {
    foreach ($name in @('prepare-engine-batch.ps1', 'focused-test-plan.ps1', 'authoring-batch.py')) {
        $source = Join-Path $sourceRepo "scripts/$name"
        if (Test-Path -LiteralPath $source) { Copy-Item -LiteralPath $source -Destination (Join-Path $fixture "scripts/$name") }
    }
    $plan = Join-Path $fixture 'focused.json'
    Set-Content -LiteralPath $plan -Value '[{"package":"tricerules-core","target":"scenario","test":"copy::rejected_choice"},{"package":"tricerules-core","target":"lib","test":"engine::identity"}]'
    $arguments = @('-FocusedTests', $plan, '-OutDirectory', 'review preparation')
    $result = Invoke-WorkflowFixture $fixture 'prepare-engine-batch.ps1' $arguments
    Assert-Workflow ($result.ExitCode -eq 0) "Engine preparation failed: $($result.Output)"
    $trace = @(Read-WorkflowTrace $fixture)
    Assert-Workflow ($trace.Count -eq 4) 'Expected format, two exact regressions and focused lint.'
    Assert-Workflow ($trace[0].Arguments -contains 'fmt') 'Format must precede compilation.'
    Assert-Workflow ($trace[1].Arguments -contains '--exact') 'Scenario must use exact selection.'
    Assert-Workflow ($trace[2].Arguments -contains '--lib') 'Library regressions must use the lib target.'
    Assert-Workflow ($trace[3].Arguments -contains 'clippy') 'Focused lint missing.'
    Assert-Workflow (@($trace | Where-Object { $_.Arguments -contains '--refresh-presentation' }).Count -eq 0) 'Engine preparation must not mutate card metadata.'
    $summary = Get-Content -LiteralPath (Join-Path $fixture 'review preparation/summary.json') -Raw | ConvertFrom-Json
    Assert-Workflow (-not $summary.semantic_approval -and $summary.final_gate -eq 'pending' -and $summary.steps.Count -eq 4) 'Preparation incorrectly granted final approval.'
    Set-Content -LiteralPath (Join-Path $fixture 'zero-tests') -Value 'fixture'
    $result = Invoke-WorkflowFixture $fixture 'prepare-engine-batch.ps1' @('-FocusedTests', $plan, '-OutDirectory', 'empty selection')
    Assert-Workflow ($result.ExitCode -ne 0 -and $result.Output -match 'Expected one executed') 'Zero-match test was accepted.'
    Remove-Item -LiteralPath (Join-Path $fixture 'zero-tests')
    [IO.File]::WriteAllText((Join-Path $fixture 'fail-pattern'), 'cargo test')
    $before = @(Read-WorkflowTrace $fixture).Count
    $result = Invoke-WorkflowFixture $fixture 'prepare-engine-batch.ps1' @('-FocusedTests', $plan, '-OutDirectory', 'failing regression')
    Assert-Workflow ($result.ExitCode -eq 7 -and $result.Output -match 'fixture complete failure log') "Failure status/log was lost: $($result.ExitCode) $($result.Output)"
    $later = @(Read-WorkflowTrace $fixture | Select-Object -Skip $before)
    Assert-Workflow (@($later | Where-Object { $_.Arguments -contains 'clippy' }).Count -eq 0) 'Failure reached lint.'
    Remove-Item -LiteralPath (Join-Path $fixture 'fail-pattern')
    $failureSummary = Get-Content -LiteralPath (Join-Path $fixture 'failing regression/summary.json') -Raw | ConvertFrom-Json
    Assert-Workflow ($failureSummary.status -eq 'failed' -and $failureSummary.steps[-1].exit_code -eq 7) 'Failed attempt was not retained.'
    Set-Content -LiteralPath $plan -Value '[{"package":"tricerules-core","target":"scenario","test":"copy::rejected_choice","features":"tricerules-cards/authoring"}]'
    $before = @(Read-WorkflowTrace $fixture).Count
    $result = Invoke-WorkflowFixture $fixture 'prepare-engine-batch.ps1' @('-FocusedTests', $plan, '-OutDirectory', 'feature coverage', '-Workers', '3')
    Assert-Workflow ($result.ExitCode -eq 0) "Feature preparation failed: $($result.Output)"
    $featureTrace = @(Read-WorkflowTrace $fixture | Select-Object -Skip $before)
    Assert-Workflow ($featureTrace.Count -eq 4 -and $featureTrace[1].Arguments -contains '--features' -and $featureTrace[3].Arguments -contains '--features') 'Feature code was not both tested and linted.'
    Assert-Workflow ($featureTrace[1].BuildJobs -eq '3' -and $featureTrace[1].TestThreads -eq '3') 'Explicit worker settings were lost.'
    foreach ($invalid in @('[{"package":"tricerules-core","target":"scenario","test":"copy::test","ignored":true}]', '[{"package":"tricerules-core","target":"scenario","test":"copy::test"},{"package":"tricerules-core","target":"scenario","test":"copy::test"}]')) {
        Set-Content -LiteralPath $plan -Value $invalid
        $before = @(Read-WorkflowTrace $fixture).Count
        $result = Invoke-WorkflowFixture $fixture 'prepare-engine-batch.ps1' @('-FocusedTests', $plan, '-OutDirectory', 'bad selection')
        Assert-Workflow ($result.ExitCode -ne 0 -and @(Read-WorkflowTrace $fixture).Count -eq $before) 'Invalid selection reached Cargo.'
    }
    Set-Content -LiteralPath $plan -Value '[]'
    $before = @(Read-WorkflowTrace $fixture).Count
    $result = Invoke-WorkflowFixture $fixture 'prepare-engine-batch.ps1' @('-FocusedTests', $plan, '-OutDirectory', 'invalid plan')
    Assert-Workflow ($result.ExitCode -ne 0 -and @(Read-WorkflowTrace $fixture).Count -eq $before) 'Invalid plan executed Cargo.'
    $result = Invoke-WorkflowFixture $fixture 'prepare-engine-batch.ps1' $arguments
    Assert-Workflow ($result.ExitCode -ne 0) 'Existing evidence directory was overwritten.'
}
finally { Remove-WorkflowFixture $fixture }
Write-Output 'PASS engine preparation exact selections, failure boundaries and evidence status'
