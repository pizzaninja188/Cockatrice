param([string] $PowerShell = (Get-Process -Id $PID).Path)
. (Join-Path $PSScriptRoot 'workflow_test_helpers.ps1')
$fixture = New-WorkflowFixture
try {
    $packageRoot = Join-Path $fixture 'tricerules/tricerules-cards'
    $targets = @()
    for ($i = 0; $i -lt 220; $i++) {
        $path = Join-Path $packageRoot ('test source with spaces ' + $i.ToString('D3') + '.rs')
        [IO.File]::WriteAllText($path, "fn main() {}`n")
        $edition = if ($i % 2 -eq 0) { '2018' } else { '2021' }
        $targets += @{ src_path = $path; edition = $edition }
    }
    @{ packages = @(@{ name = 'tricerules-cards'; targets = $targets }) } |
        ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $fixture 'format-metadata.json')
    Assert-Workflow (($targets.src_path -join ' ').Length -gt 32768) 'Fixture does not exceed the Windows command-line limit.'
    $result = Invoke-WorkflowFixture $fixture 'check-rust-format.ps1' @('-Package', 'tricerules-cards')
    Assert-Workflow ($result.ExitCode -eq 0) "Bounded formatting failed: $($result.Output)"
    $calls = @(Read-WorkflowTrace $fixture | Where-Object { $_.Tool -eq 'rustfmt' })
    Assert-Workflow ($calls.Count -gt 1) 'Formatting did not split the oversized target inventory.'
    $seen = @()
    foreach ($call in $calls) {
        Assert-Workflow ($call.Arguments -contains '--check') 'A chunk lost check mode.'
        Assert-Workflow (($call.Arguments -join ' ').Length -lt 7000) 'A chunk exceeds the documented Windows argument budget.'
        $seen += @($call.Arguments | Where-Object { $_ -like '*.rs' })
    }
    Assert-Workflow ($seen.Count -eq 220 -and @($seen | Sort-Object -Unique).Count -eq 220) 'Rust target coverage was lost or duplicated.'
    foreach ($target in $targets) {
        $call = @($calls | Where-Object { $_.Arguments -contains $target.src_path })
        Assert-Workflow ($call.Count -eq 1 -and $call[0].Arguments -contains $target.edition) 'A Cargo target or its declared edition was omitted.'
    }
    Set-Content -LiteralPath (Join-Path $fixture 'fail-pattern') -Value 'rustfmt' -NoNewline
    $before = @(Read-WorkflowTrace $fixture).Count
    $result = Invoke-WorkflowFixture $fixture 'check-rust-format.ps1' @('-Package', 'tricerules-cards')
    Assert-Workflow ($result.ExitCode -eq 7 -and $result.Output -match 'fixture complete failure log') 'Formatting failure lost its exit code or full log.'
    Assert-Workflow (@(Read-WorkflowTrace $fixture).Count -eq $before + 2) 'Formatting continued after a failed chunk.'
    Set-Content -LiteralPath (Join-Path $fixture 'fail-pattern') -Value 'cargo metadata' -NoNewline
    $result = Invoke-WorkflowFixture $fixture 'check-rust-format.ps1' @('-Package', 'tricerules-cards')
    Assert-Workflow ($result.ExitCode -eq 7) 'Cargo discovery failure status was lost.'
    Remove-Item -LiteralPath (Join-Path $fixture 'fail-pattern')
    @{ packages = @(@{ name = 'tricerules-cards'; targets = @() }) } |
        ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $fixture 'format-metadata.json')
    $result = Invoke-WorkflowFixture $fixture 'check-rust-format.ps1' @('-Package', 'tricerules-cards')
    Assert-Workflow ($result.ExitCode -ne 0) 'An empty Cargo target inventory passed formatting.'
    Write-Output 'Rust formatting workflow tests passed.'
}
finally { Remove-WorkflowFixture $fixture }
