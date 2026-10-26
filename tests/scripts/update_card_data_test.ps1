param([string] $PowerShell = (Get-Process -Id $PID).Path)
. (Join-Path $PSScriptRoot 'workflow_test_helpers.ps1')
$fixture = New-WorkflowFixture
try {
    $arguments = @('-OracleBulk', 'oracle-cards.jsonl.gz', '-CardsXml', 'cards.xml')
    $checklist = Join-Path $fixture 'tricerules\CARDS.md'
    $fingerprint = Join-Path $fixture 'fingerprint'
    $ron = Join-Path $fixture 'tricerules\tricerules-cards\data\fixture.ron'
    New-Item -ItemType Directory -Path (Split-Path -Parent $ron) -Force | Out-Null
    [IO.File]::WriteAllText($ron, '(fixture: true)')
    $before = (Get-Item -LiteralPath $checklist).LastWriteTimeUtc
    $beforeChecklistBytes = [IO.File]::ReadAllBytes($checklist)
    $beforeFingerprintBytes = [IO.File]::ReadAllBytes($fingerprint)
    $beforeRonBytes = [IO.File]::ReadAllBytes($ron)
    $result = Invoke-WorkflowFixture $fixture 'update-card-data.ps1' $arguments (Join-Path $fixture 'nested directory')
    Assert-Workflow ($result.ExitCode -eq 0) "Canonical check failed: $($result.Output)"
    Assert-Workflow ((Get-Item -LiteralPath $checklist).LastWriteTimeUtc -eq $before) 'Check rewrote CARDS.md.'
    $trace = @(Read-WorkflowTrace $fixture)
    Assert-Workflow ($trace.Count -eq 2) 'Check did not run source metadata and checklist gates after review evidence.'
    foreach ($call in $trace) {
        Assert-Workflow ($call.Arguments -contains '--check') 'Check invoked a mutating generator mode.'
        Assert-Workflow ($call.Cwd -eq $fixture) 'Source paths did not resolve from the checkout.'
    }
    $authorCheck = @($trace | Where-Object { $_.Arguments -contains '--bin' -and $_.Arguments -contains 'card-author' })
    Assert-Workflow ($authorCheck.Count -eq 1 -and $authorCheck[0].Arguments -contains 'metadata') 'CardData Check did not route source/presentation validation through card-author.'
    Assert-Workflow (@($trace | Where-Object { $_.Arguments -contains '--bin' -and $_.Arguments -contains 'gen-cards' }).Count -eq 0) 'Check invoked the retired generator.'
    Assert-Workflow ([BitConverter]::ToString($beforeChecklistBytes) -ceq [BitConverter]::ToString([IO.File]::ReadAllBytes($checklist))) 'Check changed CARDS.md bytes.'
    Assert-Workflow ([BitConverter]::ToString($beforeFingerprintBytes) -ceq [BitConverter]::ToString([IO.File]::ReadAllBytes($fingerprint))) 'Check changed fingerprint bytes.'
    Assert-Workflow ([BitConverter]::ToString($beforeRonBytes) -ceq [BitConverter]::ToString([IO.File]::ReadAllBytes($ron))) 'Check changed RON bytes.'

    [IO.File]::WriteAllText($checklist, 'stale checklist')
    $result = Invoke-WorkflowFixture $fixture 'update-card-data.ps1' $arguments
    Assert-Workflow ($result.ExitCode -ne 0) 'Checklist drift was not detected.'
    Assert-Workflow ([IO.File]::ReadAllText($checklist) -eq 'stale checklist') 'Check repaired drift implicitly.'

    [IO.File]::WriteAllText($fingerprint, 'stale')
    $result = Invoke-WorkflowFixture $fixture 'update-card-data.ps1' $arguments
    Assert-Workflow ($result.ExitCode -eq 9) 'Fingerprint failure exit code was lost.'
    Assert-Workflow ($result.Output -match 'fingerprint drift') 'Fingerprint failure log was hidden.'

    $refreshStart = @(Read-WorkflowTrace $fixture).Count
    $result = Invoke-WorkflowFixture $fixture 'update-card-data.ps1' (@('-Mode', 'Refresh') + $arguments)
    Assert-Workflow ($result.ExitCode -eq 0) 'Metadata Refresh failed.'
    $refreshCalls = @(Read-WorkflowTrace $fixture | Select-Object -Skip $refreshStart)
    Assert-Workflow ($refreshCalls.Count -eq 2) 'Refresh did not update metadata and validate the checklist once.'
    Assert-Workflow ($refreshCalls[0].Arguments -contains 'card-author' -and $refreshCalls[0].Arguments -contains 'metadata' -and $refreshCalls[0].Arguments -contains '--refresh') 'Refresh did not route fingerprint refresh through card-author.'
    Assert-Workflow ([BitConverter]::ToString($beforeRonBytes) -ceq [BitConverter]::ToString([IO.File]::ReadAllBytes($ron))) 'Refresh changed direct-RON card data.'
    $result = Invoke-WorkflowFixture $fixture 'update-card-data.ps1' $arguments
    Assert-Workflow ($result.ExitCode -eq 0) "Check after Refresh failed: $($result.Output)"

    Set-Content -LiteralPath (Join-Path $fixture 'bad-names') -Value 'invalid'
    [IO.File]::WriteAllText($checklist, 'preserve this checklist')
    $result = Invoke-WorkflowFixture $fixture 'update-card-data.ps1' (@('-Mode', 'Refresh') + $arguments)
    Assert-Workflow ($result.ExitCode -eq 11) 'Name-validation failure exit code was lost.'
    Assert-Workflow ([IO.File]::ReadAllText($checklist) -eq 'preserve this checklist') 'Failed validation overwrote checklist.'
    Assert-Workflow ($result.Output -match 'unmatched card name') 'Name-validation failure log was hidden.'

    Set-Content -LiteralPath (Join-Path $fixture 'bad-evidence') -Value 'invalid'
    $result = Invoke-WorkflowFixture $fixture 'update-card-data.ps1' $arguments
    Assert-Workflow ($result.ExitCode -eq 13) 'Evidence failure exit code was lost.'
    Assert-Workflow ($result.Output -match 'invalid evidence reference') 'Evidence failure log was hidden.'

    $callCount = @(Read-WorkflowTrace $fixture).Count
    $result = Invoke-WorkflowFixture $fixture 'update-card-data.ps1' @('-CardsXml', 'missing.xml')
    Assert-Workflow ($result.ExitCode -ne 0) 'Missing input was accepted.'
    Assert-Workflow (@(Read-WorkflowTrace $fixture).Count -eq $callCount) 'Missing inputs were not checked before execution.'
    $result = Invoke-WorkflowFixture $fixture 'update-card-data.ps1' @('-OracleBulk', 'missing.gz', '-CardsXml', 'cards.xml')
    Assert-Workflow ($result.ExitCode -ne 0) 'Missing bulk input was accepted.'
    Assert-Workflow (@(Read-WorkflowTrace $fixture).Count -eq $callCount) 'Missing bulk input reached a tool.'
}
finally { Remove-WorkflowFixture $fixture }
Write-Output 'PASS card data workflow regression'
