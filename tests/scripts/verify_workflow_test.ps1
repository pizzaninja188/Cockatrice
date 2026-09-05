param([string] $PowerShell = (Get-Process -Id $PID).Path)
. (Join-Path $PSScriptRoot 'workflow_test_helpers.ps1')
$fixture = New-WorkflowFixture
try {
    $nested = Join-Path $fixture 'nested directory'
    $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Both', '-CardData', '-Preview') $nested
    Assert-Workflow ($result.ExitCode -eq 0) "Preview failed: $($result.Output)"
    Assert-Workflow (@(Read-WorkflowTrace $fixture).Count -eq 0) 'Preview executed a command.'
    Assert-Workflow (-not (Test-Path -LiteralPath (Join-Path $fixture 'build'))) 'Preview created build artifacts.'
    Assert-Workflow ($result.Output -match 'Rust tests' -and $result.Output -match 'Windows Ninja build' -and $result.Output -match 'Card data') 'Preview omitted gates.'

    $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Cpp', '-CardData')
    Assert-Workflow ($result.ExitCode -ne 0) 'Cpp-only card verification was accepted.'
    Assert-Workflow (@(Read-WorkflowTrace $fixture).Count -eq 0) 'Invalid options executed commands.'

    $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Both', '-CardData') $nested
    Assert-Workflow ($result.ExitCode -eq 0) "Combined verification failed: $($result.Output)"
    Assert-Workflow ($result.Output -notmatch 'fixture successful (stdout|stderr)') 'Successful command output was noisy.'
    $trace = @(Read-WorkflowTrace $fixture)
    Assert-Workflow (($trace.Tool -join ',') -eq 'cargo,cargo,cargo,build,ctest,cargo,cargo,git') 'Wrong combined gate order.'
    foreach ($call in $trace[0..2]) {
        Assert-Workflow ($call.Cwd -eq (Join-Path $fixture 'tricerules')) 'Rust command ran outside tricerules.'
    }
    Assert-Workflow ($trace[4].RequireE2E -eq '1') 'CTest did not require E2E prerequisites.'
    Assert-Workflow ($trace[4].Arguments -contains '--no-tests=error') 'CTest could accept an empty suite.'
    $summaries = @(Get-ChildItem -LiteralPath (Join-Path $fixture 'build\verification-logs') -Filter summary.json -Recurse)
    Assert-Workflow ($summaries.Count -eq 1) 'Combined run did not save one summary.'
    $summary = Get-Content -LiteralPath $summaries[0].FullName -Raw | ConvertFrom-Json
    Assert-Workflow ($summary.ExitCode -eq 0 -and $summary.Status -eq 'Pass') 'Summary did not report success.'
    Assert-Workflow ($summary.Steps.Count -eq 7) 'Summary omitted a gate.'
    foreach ($step in $summary.Steps) {
        Assert-Workflow ($step.Status -eq 'Pass' -and $step.ExitCode -eq 0) 'Summary reported a successful gate incorrectly.'
        Assert-Workflow (Test-Path -LiteralPath $step.LogPath) 'Summary points to a missing log.'
        Assert-Workflow ($step.Executable -and $step.WorkingDirectory -and $step.Arguments.Count -gt 0) 'Summary lacks reproducible command details.'
    }

    # Fail in the second Rust gate. Neither formatting nor later gates may execute.
    Set-Content -LiteralPath (Join-Path $fixture 'fail-pattern') -Value 'cargo clippy' -NoNewline
    $count = $trace.Count
    $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Rust')
    Assert-Workflow ($result.ExitCode -eq 7) "Verification lost failure code: $($result.Output)"
    Assert-Workflow ($result.Output -match 'fixture complete failure log') 'Verification hid the full failure log.'
    Assert-Workflow (@(Read-WorkflowTrace $fixture).Count -eq ($count + 2)) 'Verification continued after failure.'
    Assert-Workflow ($result.Output -match 'NOT RUN') 'Unexecuted gates were not identified.'
    $failureSummary = Get-ChildItem -LiteralPath (Join-Path $fixture 'build\verification-logs') -Filter summary.json -Recurse |
        ForEach-Object { Get-Content -LiteralPath $_.FullName -Raw | ConvertFrom-Json } |
        Where-Object { $_.ExitCode -eq 7 }
    Assert-Workflow ($failureSummary.Steps[1].Status -eq 'Fail' -and $failureSummary.Steps[2].Status -eq 'NotRun') 'Failure summary is misleading.'

    # Check restoration in the same PowerShell process, on success and failure.
    Set-Content -LiteralPath (Join-Path $fixture 'scripts\environment-probe.ps1') -Value @'
$env:RULED_E2E_REQUIRE = 'caller-value'
& (Join-Path $PSScriptRoot 'verify.ps1') -Side Cpp
$code = $LASTEXITCODE
if ($env:RULED_E2E_REQUIRE -ne 'caller-value') { throw 'E2E environment was not restored' }
exit $code
'@
    $result = Invoke-WorkflowFixture $fixture 'environment-probe.ps1'
    Assert-Workflow ($result.ExitCode -eq 0) "Cpp gate or environment restoration failed: $($result.Output)"
    Set-Content -LiteralPath (Join-Path $fixture 'fail-pattern') -Value 'ctest' -NoNewline
    $result = Invoke-WorkflowFixture $fixture 'environment-probe.ps1'
    Assert-Workflow ($result.ExitCode -eq 7) "Failed CTest environment restoration failed: $($result.Output)"
}
finally { Remove-WorkflowFixture $fixture }
Write-Output 'PASS verification workflow regression'
