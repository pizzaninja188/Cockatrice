param([string] $PowerShell = (Get-Process -Id $PID).Path)
. (Join-Path $PSScriptRoot 'workflow_test_helpers.ps1')
$fixture = New-WorkflowFixture
try {
    $arguments = @('-OracleBulk', 'oracle-cards.jsonl.gz', '-CardsXml', 'cards.xml')
    $checklist = Join-Path $fixture 'tricerules\CARDS.md'
    $fingerprint = Join-Path $fixture 'fingerprint'
    $before = (Get-Item -LiteralPath $checklist).LastWriteTimeUtc
    $result = Invoke-WorkflowFixture $fixture 'update-card-data.ps1' $arguments (Join-Path $fixture 'nested directory')
    Assert-Workflow ($result.ExitCode -eq 0) "Canonical check failed: $($result.Output)"
    Assert-Workflow ((Get-Item -LiteralPath $checklist).LastWriteTimeUtc -eq $before) 'Check rewrote CARDS.md.'
    $trace = @(Read-WorkflowTrace $fixture)
    Assert-Workflow ($trace.Count -eq 2) 'Check did not run both generators.'
    foreach ($call in $trace) {
        Assert-Workflow ($call.Arguments -contains '--check') 'Check invoked a mutating generator mode.'
        Assert-Workflow ($call.Cwd -eq $fixture) 'Source paths did not resolve from the checkout.'
    }

    [IO.File]::WriteAllText($checklist, 'stale checklist')
    $result = Invoke-WorkflowFixture $fixture 'update-card-data.ps1' $arguments
    Assert-Workflow ($result.ExitCode -ne 0) 'Checklist drift was not detected.'
    Assert-Workflow ([IO.File]::ReadAllText($checklist) -eq 'stale checklist') 'Check repaired drift implicitly.'

    [IO.File]::WriteAllText($fingerprint, 'stale')
    $result = Invoke-WorkflowFixture $fixture 'update-card-data.ps1' $arguments
    Assert-Workflow ($result.ExitCode -eq 9) 'Fingerprint failure exit code was lost.'
    Assert-Workflow ($result.Output -match 'fingerprint drift') 'Fingerprint failure log was hidden.'

    $result = Invoke-WorkflowFixture $fixture 'update-card-data.ps1' (@('-Mode', 'Refresh') + $arguments)
    Assert-Workflow ($result.ExitCode -eq 0) "Refresh failed: $($result.Output)"
    Assert-Workflow ([IO.File]::ReadAllText($checklist) -eq 'canonical checklist') 'Refresh did not publish validated checklist.'
    Assert-Workflow ([IO.File]::ReadAllText($fingerprint) -eq 'canonical') 'Refresh did not refresh fingerprints.'
    $result = Invoke-WorkflowFixture $fixture 'update-card-data.ps1' $arguments
    Assert-Workflow ($result.ExitCode -eq 0) "Check after Refresh failed: $($result.Output)"

    Set-Content -LiteralPath (Join-Path $fixture 'bad-names') -Value 'invalid'
    [IO.File]::WriteAllText($checklist, 'preserve this checklist')
    $result = Invoke-WorkflowFixture $fixture 'update-card-data.ps1' (@('-Mode', 'Refresh') + $arguments)
    Assert-Workflow ($result.ExitCode -eq 11) 'Name-validation failure exit code was lost.'
    Assert-Workflow ([IO.File]::ReadAllText($checklist) -eq 'preserve this checklist') 'Failed validation overwrote checklist.'
    Assert-Workflow ($result.Output -match 'unmatched card name') 'Name-validation failure log was hidden.'

    $callCount = @(Read-WorkflowTrace $fixture).Count
    $result = Invoke-WorkflowFixture $fixture 'update-card-data.ps1' @('-CardsXml', 'missing.xml')
    Assert-Workflow ($result.ExitCode -ne 0) 'Missing input was accepted.'
    Assert-Workflow (@(Read-WorkflowTrace $fixture).Count -eq $callCount) 'Missing inputs were not checked before execution.'
    $result = Invoke-WorkflowFixture $fixture 'update-card-data.ps1' @('-OracleBulk', 'missing.gz', '-CardsXml', 'cards.xml')
    Assert-Workflow ($result.ExitCode -ne 0) 'Missing bulk input was accepted.'
    Assert-Workflow (@(Read-WorkflowTrace $fixture).Count -eq $callCount) 'Missing bulk input reached a generator.'
    $expanded = @(Read-WorkflowTrace $fixture | Where-Object { $_.Arguments -contains '--include-new' })
    Assert-Workflow ($expanded.Count -eq 0) 'Refresh expanded the card set.'
}
finally { Remove-WorkflowFixture $fixture }
Write-Output 'PASS card data workflow regression'
