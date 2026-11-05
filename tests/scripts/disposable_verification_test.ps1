param([string] $PowerShell = (Get-Process -Id $PID).Path)
. (Join-Path $PSScriptRoot 'workflow_test_helpers.ps1')
$fixture = New-WorkflowFixture
try {
    $marker = Join-Path $fixture 'disposable-fixture'
    Set-Content -LiteralPath $marker -Value 'linked'
    $target = Join-Path $fixture 'tricerules/target'
    $preset = if ($workflowWindows) { 'windows-ninja-all' } else { 'linux-ninja-all' }
    $nativeBuild = Join-Path $fixture "build/$preset"
    $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Both', '-DisposableBuild', '-Preview')
    Assert-Workflow ($result.ExitCode -eq 0 -and $result.Output -match 'CARGO_INCREMENTAL=0') "Disposable preview failed: $($result.Output)"
    Assert-Workflow (@(Read-WorkflowTrace $fixture).Count -eq 0) 'Disposable preview executed commands.'

    New-Item -ItemType Directory -Path $target | Out-Null
    $preExisting = Join-Path $target 'keep'
    Set-Content -LiteralPath $preExisting -Value 'existing'
    $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Both', '-DisposableBuild')
    Assert-Workflow ($result.ExitCode -ne 0 -and $result.Output -match 'existing build output') "Existing build output was accepted: $($result.Output)"
    Assert-Workflow (Test-Path -LiteralPath $preExisting) 'Existing build output was removed.'
    Remove-Item -LiteralPath $preExisting
    Remove-Item -LiteralPath $target

    $fail = Join-Path $fixture 'fail-pattern'
    Set-Content -LiteralPath $fail -Value '^ctest ' -NoNewline
    $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Both', '-DisposableBuild')
    Assert-Workflow ($result.ExitCode -eq 7) "Disposable verification lost the test exit code: $($result.Output)"
    Assert-Workflow (-not (Test-Path -LiteralPath $target) -and -not (Test-Path -LiteralPath $nativeBuild)) "Disposable outputs survived a test failure: $($result.Output)"
    $trace = @(Read-WorkflowTrace $fixture | Where-Object Tool -in @('cargo','build','ctest'))
    Assert-Workflow ($trace.Count -gt 0 -and @($trace | Where-Object {
        $_.Incremental -ne '0' -or $_.CargoTarget -ne $target -or $_.CargoBuildDir -ne $target
    }).Count -eq 0) 'A disposable step inherited the caller cache configuration.'
    Assert-Workflow (Test-Path -LiteralPath (Join-Path $fixture 'build/verification-logs')) 'Disposable cleanup removed logs.'

    Remove-Item -LiteralPath $fail
    $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Both', '-DisposableBuild')
    Assert-Workflow ($result.ExitCode -eq 0 -and -not (Test-Path -LiteralPath $target) -and -not (Test-Path -LiteralPath $nativeBuild)) "Disposable success did not clean up: $($result.Output)"
    Assert-Workflow (Test-Path -LiteralPath (Join-Path $fixture 'tricerules/Cargo.toml')) 'Disposable cleanup removed source.'

    Remove-Item -LiteralPath $marker
    $result = Invoke-WorkflowFixture $fixture 'verify.ps1' @('-Side', 'Rust', '-DisposableBuild')
    Assert-Workflow ($result.ExitCode -ne 0 -and $result.Output -match 'linked Git worktree') 'A primary checkout was accepted.'
}
finally { Remove-WorkflowFixture $fixture }
Write-Output 'PASS disposable verification lifecycle'
