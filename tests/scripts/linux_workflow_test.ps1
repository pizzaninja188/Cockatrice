param([string] $PowerShell = (Get-Process -Id $PID).Path)
. (Join-Path $PSScriptRoot 'workflow_test_helpers.ps1')
$fixture = New-WorkflowFixture
try {
    # Exercise the real Linux wrapper with a recording native cmake, on either host.
    Copy-Item -LiteralPath (Join-Path $sourceRepo 'scripts/build-linux.ps1') -Destination (Join-Path $fixture 'scripts/build-linux.ps1')
    $result = Invoke-WorkflowFixture $fixture 'build-linux.ps1' @('--target', 'servatrice') (Join-Path $fixture 'nested directory')
    Assert-Workflow ($result.ExitCode -eq 0) "Linux build wrapper failed: $($result.Output)"
    $trace = @(Read-WorkflowTrace $fixture)
    Assert-Workflow ($trace.Count -eq 2) 'Expected configure then build.'
    Assert-Workflow (($trace[0].Arguments -join ',') -eq '--preset,linux-ninja-all') 'Linux configure used the wrong preset.'
    Assert-Workflow (($trace[1].Arguments -join ',') -eq '--build,--preset,linux-ninja-all,--target,servatrice') 'Linux build lost target arguments.'
    Assert-Workflow (@($trace | Where-Object Cwd -ne $fixture).Count -eq 0) 'Linux build depended on caller cwd.'
    Set-Content -LiteralPath (Join-Path $fixture 'fail-pattern') -Value 'cmake --preset' -NoNewline
    $result = Invoke-WorkflowFixture $fixture 'build-linux.ps1'
    Assert-Workflow ($result.ExitCode -eq 7) 'Configure failure code was lost.'
    Assert-Workflow (@(Read-WorkflowTrace $fixture).Count -eq 3) 'Build ran after configure failure.'
    Set-Content -LiteralPath (Join-Path $fixture 'fail-pattern') -Value 'cmake --build' -NoNewline
    $result = Invoke-WorkflowFixture $fixture 'build-linux.ps1'
    Assert-Workflow ($result.ExitCode -eq 7) 'Build failure code was lost.'
}
finally { Remove-WorkflowFixture $fixture }
Write-Output 'PASS Linux configure/build order and exact failure propagation'
