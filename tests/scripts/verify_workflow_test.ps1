param([string] $PowerShell = (Get-Process -Id $PID).Path)
. (Join-Path $PSScriptRoot 'workflow_test_helpers.ps1')
$fixture = New-WorkflowFixture
try {
    # Invalid card evidence must stop before the full Rust suite or C++ work.
    Set-Content -LiteralPath (Join-Path $fixture 'bad-evidence') -Value 'invalid'
    $earlyFailure = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Both', '-CardData')
    Assert-Workflow ($earlyFailure.ExitCode -eq 13) 'Card evidence failure code was lost.'
    Assert-Workflow (@(Read-WorkflowTrace $fixture).Count -eq 0) 'Expensive gates ran before invalid card evidence was rejected.'
    $earlySummary = Get-ChildItem -LiteralPath (Join-Path $fixture 'build/verification-logs') -Filter summary.json -Recurse |
        ForEach-Object { Get-Content -LiteralPath $_.FullName -Raw | ConvertFrom-Json }
    Assert-Workflow ($earlySummary.Steps[0].Label -eq 'Card data' -and $earlySummary.Steps[0].Status -eq 'Fail') 'Card gate was not first.'
    foreach ($step in $earlySummary.Steps | Select-Object -Skip 1) {
        Assert-Workflow ($step.Status -eq 'NotRun') 'Failed card gate did not stop verification.'
    }
    Remove-Item -LiteralPath (Join-Path $fixture 'bad-evidence')
    [IO.File]::WriteAllText((Join-Path $fixture 'fingerprint'), 'drift')
    $earlyFailure = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Rust', '-CardData')
    Assert-Workflow ($earlyFailure.ExitCode -eq 9) 'Generated-data drift failure code was lost.'
    $earlyTrace = @(Read-WorkflowTrace $fixture)
    Assert-Workflow ($earlyTrace.Count -eq 1 -and $earlyTrace[0].Arguments -contains 'gen-cards' -and $earlyTrace[0].Arguments -contains '--check') 'Full suites ran before generated-data drift was rejected.'
    Remove-WorkflowFixture $fixture
    $fixture = New-WorkflowFixture
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
    $formatCalls = @($trace | Where-Object { $_.Tool -eq 'rustfmt' })
    Assert-Workflow ($formatCalls.Count -eq 5) 'Each Rust workspace package must receive a separate formatting command to avoid Windows command-line limits.'
    foreach ($package in @('tricerules-proto', 'tricerules-core', 'tricerules-card-model', 'tricerules-cards', 'tricerules-server')) {
        Assert-Workflow (@($formatCalls | Where-Object { ($_.Arguments -join ' ') -match $package }).Count -eq 1) "Formatting did not cover exactly one $package package."
    }
    Assert-Workflow (($trace.Tool -join ',') -eq 'cargo,cargo,cargo,cargo,cargo,rustfmt,cargo,rustfmt,cargo,rustfmt,cargo,rustfmt,cargo,rustfmt,build,ctest,git') 'Wrong combined gate order.'
    foreach ($call in $trace[2..13]) {
        Assert-Workflow ($call.Cwd -eq (Join-Path $fixture 'tricerules')) 'Rust command ran outside tricerules.'
    }
    Assert-Workflow ($trace[15].RequireE2E -eq '1') 'CTest did not require E2E prerequisites.'
    Assert-Workflow ($trace[15].Arguments -contains '--no-tests=error') 'CTest could accept an empty suite.'
    $summaries = @(Get-ChildItem -LiteralPath (Join-Path $fixture 'build\verification-logs') -Filter summary.json -Recurse)
    Assert-Workflow ($summaries.Count -eq 1) 'Combined run did not save one summary.'
    $summary = Get-Content -LiteralPath $summaries[0].FullName -Raw | ConvertFrom-Json
    Assert-Workflow ($summary.ExitCode -eq 0 -and $summary.Status -eq 'Pass') 'Summary did not report success.'
    Assert-Workflow ($summary.Steps.Count -eq 12) 'Summary omitted a gate.'
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
    Assert-Workflow ($failureSummary.Steps[2].Status -eq 'Fail' -and $failureSummary.Steps[3].Status -eq 'NotRun') 'Failure summary is misleading.'

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

    # A package formatting failure stops the remaining packages and preserves its status.
    Set-Content -LiteralPath (Join-Path $fixture 'fail-pattern') -Value 'rustfmt.*tricerules-core' -NoNewline
    $before = @(Read-WorkflowTrace $fixture).Count
    $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Rust')
    Assert-Workflow ($result.ExitCode -eq 7 -and $result.Output -match 'fixture complete failure log') 'Package formatting failure lost its log or status.'
    $new = @(Read-WorkflowTrace $fixture | Select-Object -Skip $before)
    Assert-Workflow ($new.Count -eq 6) 'Verification continued beyond the failing second package format check.'
    Assert-Workflow (($new[5].Arguments -join ' ') -match 'tricerules-core') 'The expected formatting package did not fail.'

    # Preview derives coverage from the workspace, including newly added packages, without tools.
    $extra = Join-Path $fixture 'tricerules/extra-member'
    New-Item -ItemType Directory -Path $extra | Out-Null
    [IO.File]::WriteAllText((Join-Path $extra 'Cargo.toml'), "[package]`nname = `"extra-format-package`"`n")
    $workspaceManifest = Join-Path $fixture 'tricerules/Cargo.toml'
    $workspaceText = [IO.File]::ReadAllText($workspaceManifest)
    $workspaceText = $workspaceText.Replace('    "tricerules-server",', "    `"tricerules-server`",`n    `"extra-member`",")
    [IO.File]::WriteAllText($workspaceManifest, $workspaceText)
    $before = @(Read-WorkflowTrace $fixture).Count
    $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Rust', '-Preview')
    Assert-Workflow ($result.ExitCode -eq 0 -and $result.Output -match 'extra-format-package') 'Preview omitted the added workspace package.'
    Assert-Workflow (@(Read-WorkflowTrace $fixture).Count -eq $before) 'Package discovery executed a command during preview.'
    [IO.File]::WriteAllText($workspaceManifest, $workspaceText.Replace('    "tricerules-proto",', '    "tricerules-proto", # ] comment must not end the member list'))
    $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Rust', '-Preview')
    foreach ($package in @('tricerules-proto', 'tricerules-core', 'tricerules-card-model', 'tricerules-cards', 'tricerules-server', 'extra-format-package')) {
        Assert-Workflow ($result.ExitCode -eq 0 -and $result.Output -match $package) "A comment containing a bracket silently omitted $package from formatting."
    }
    Assert-Workflow (@(Read-WorkflowTrace $fixture).Count -eq $before) 'Comment-aware discovery executed commands in preview.'
    foreach ($invalid in @('[package]', "[package]`nname = `"tricerules-proto`"`n")) {
        [IO.File]::WriteAllText((Join-Path $extra 'Cargo.toml'), $invalid)
        $before = @(Read-WorkflowTrace $fixture).Count
        $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Rust')
        Assert-Workflow ($result.ExitCode -ne 0 -and @(Read-WorkflowTrace $fixture).Count -eq $before) 'Missing or duplicate package identity executed an incomplete formatting plan.'
    }
    [IO.File]::WriteAllText($workspaceManifest, "[workspace]`nmembers = []`n")
    $before = @(Read-WorkflowTrace $fixture).Count
    $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Rust')
    Assert-Workflow ($result.ExitCode -ne 0 -and @(Read-WorkflowTrace $fixture).Count -eq $before) 'Empty workspace membership reached verification commands.'
}
finally { Remove-WorkflowFixture $fixture }
Write-Output 'PASS verification workflow regression'
