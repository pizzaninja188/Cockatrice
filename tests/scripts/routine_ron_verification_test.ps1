param([string] $PowerShell = (Get-Process -Id $PID).Path)
. (Join-Path $PSScriptRoot 'workflow_test_helpers.ps1')
$implementation = Join-Path $sourceRepo 'scripts/routine-ron-checks.ps1'
Assert-Workflow (Test-Path -LiteralPath $implementation -PathType Leaf) 'Missing routine RON eligibility and test selection implementation.'
. $implementation

function Assert-Rejected {
    param([scriptblock] $Action, [string] $Message)
    $rejected = $false
    try { $null = & $Action } catch { $rejected = $true }
    Assert-Workflow $rejected $Message
}

$fixture = New-WorkflowFixture
try {
    # Inventory the fixture as Git would. The independent real-gate run checks the
    # native Git integration; this keeps the fixture usable under .git write restrictions.
    $fakeGitlinkId = 'a' * 40
    $fakeGitlinkDirty = $false
    function git {
        if ($args -contains '--stage') {
            $global:LASTEXITCODE = 0
            "160000 $fakeGitlinkId 0`tvendor/test`0"
            return
        }
        if ($args -contains 'status') {
            $global:LASTEXITCODE = 0
            if ($fakeGitlinkDirty) { ' M vendor/test' }
            return
        }
        $names = @(Get-ChildItem -LiteralPath $fixture -File -Recurse -Force | ForEach-Object {
            $_.FullName.Substring($fixture.Length + 1).Replace('\', '/')
        } | Where-Object { $_ -notmatch '(^|/)build/' -and $_ -notmatch '(^|/)ignored[^/]*\.(ron|rs)$' })
        $names += 'vendor/test'
        $global:LASTEXITCODE = 0
        [string]::Join("`0", $names) + "`0"
    }
    $data = Join-Path $fixture 'tricerules/tricerules-cards/data'
    $maps = Join-Path $fixture 'tricerules/tricerules-cards/authoring/review-maps'
    $scenarios = Join-Path $fixture 'tricerules/tricerules-core/tests/scenario'
    $cardsTests = Join-Path $fixture 'tricerules/tricerules-cards/tests'
    foreach ($dir in @($data, $maps, $scenarios, $cardsTests)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
    [IO.File]::WriteAllText((Join-Path $fixture '.gitignore'), "build/`ntricerules/target/`nignored*.ron`n")
    [IO.File]::WriteAllText((Join-Path $data 'other.ron'), 'old card')
    [IO.File]::WriteAllText((Join-Path $scenarios 'example.rs'), 'old tests')
    [IO.File]::WriteAllText((Join-Path $cardsTests 'example.rs'), 'old card tests')
    [IO.File]::WriteAllText((Join-Path $fixture 'tricerules/tricerules-core/tests/scenario.rs'), 'harness')
    $engineDir = Join-Path $fixture 'tricerules/tricerules-core/src'
    New-Item -ItemType Directory -Force -Path $engineDir | Out-Null
    [IO.File]::WriteAllText((Join-Path $engineDir 'engine.rs'), 'verified engine')
    [IO.File]::WriteAllText((Join-Path $engineDir 'ignored_compiled.rs'), 'verified ignored source')
    $baseline = Get-VerificationInputSnapshot -Repository $fixture

    $card = Join-Path $data 'example.ron'
    $map = Join-Path $maps 'example.json'
    [IO.File]::WriteAllText($card, 'new card')
    $definition = [ordered]@{ tokens = @(); semantic_fixtures = @(
        @{ test = 'scenario example::actual_card'; covers = 'actual behavior' },
        @{ test = 'batch_registry::example::definition'; covers = 'card definition' }
    ); complete_definition_review_confirmed = $true }
    $definition | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $map
    [IO.File]::WriteAllText((Join-Path $scenarios 'example.rs'), 'old tests plus actual-card assertion')
    [IO.File]::WriteAllText((Join-Path $cardsTests 'example.rs'), 'old card tests plus assertion')
    $current = Get-VerificationInputSnapshot -Repository $fixture
    $plan = Get-RoutineRonPlan -Repository $fixture -Baseline $baseline -Current $current
    Assert-Workflow (@($plan.Cards).Count -eq 1 -and $plan.Cards[0] -eq 'example') 'The changed card was not selected.'
    Assert-Workflow (@($plan.Tests).Count -eq 2) 'All referenced semantic tests must be selected.'
    Assert-Workflow (@($plan.Tests | Where-Object { $_.Package -eq 'tricerules-core' -and $_.Target -eq 'scenario' -and $_.Name -eq 'example::actual_card' }).Count -eq 1) 'The actual-card scenario was not selected exactly.'
    Assert-Workflow (@($plan.Tests | Where-Object { $_.Package -eq 'tricerules-cards' -and $_.Target -eq 'batch_registry' -and $_.Name -eq 'example::definition' }).Count -eq 1) 'The registry assertion was not selected exactly.'

    foreach ($path in @('tricerules/tricerules-core/src/new.rs', 'tricerules/tricerules-cards/data/tokens/token.ron',
            'tricerules/tricerules-core/tests/scenario/helpers.rs', 'tricerules/tricerules-core/tests/scenario.rs')) {
        $absolute = Join-Path $fixture $path
        $parent = Split-Path -Parent $absolute
        if (-not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Force -Path $parent | Out-Null }
        $original = if (Test-Path -LiteralPath $absolute) { [IO.File]::ReadAllText($absolute) } else { $null }
        [IO.File]::WriteAllText($absolute, 'out-of-scope change')
        $candidate = Get-VerificationInputSnapshot -Repository $fixture
        Assert-Rejected { Get-RoutineRonPlan -Repository $fixture -Baseline $baseline -Current $candidate } "Accepted out-of-scope change: $path"
        if ($null -eq $original) { Remove-Item -LiteralPath $absolute } else { [IO.File]::WriteAllText($absolute, $original) }
    }
    # build.rs includes ignored RON files too: they cannot disappear from the comparison.
    $ignored = Join-Path $data 'ignored_hidden.ron'
    [IO.File]::WriteAllText($ignored, 'ignored but compiled')
    $ignoredMap = Join-Path $maps 'ignored_hidden.json'
    $definition | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $ignoredMap
    $candidate = Get-VerificationInputSnapshot -Repository $fixture
    $failure = $null
    try { $null = Get-RoutineRonPlan -Repository $fixture -Baseline $baseline -Current $candidate } catch { $failure = $_.ToString() }
    Assert-Workflow ($failure -match 'Ignored input change') 'Accepted ignored embedded RON with an otherwise valid review map.'
    Remove-Item -LiteralPath $ignored
    Remove-Item -LiteralPath $ignoredMap

    # The Git inventory is identical across this simulated committed edit; comparing
    # snapshots must still notice that the verified engine bytes changed.
    [IO.File]::WriteAllText((Join-Path $engineDir 'engine.rs'), 'changed and committed engine')
    $candidate = Get-VerificationInputSnapshot -Repository $fixture
    Assert-Rejected { Get-RoutineRonPlan -Repository $fixture -Baseline $baseline -Current $candidate } 'Accepted an engine change already present in the Git inventory.'
    [IO.File]::WriteAllText((Join-Path $engineDir 'engine.rs'), 'verified engine')
    [IO.File]::WriteAllText((Join-Path $engineDir 'ignored_compiled.rs'), 'changed ignored source')
    $candidate = Get-VerificationInputSnapshot -Repository $fixture
    Assert-Rejected { Get-RoutineRonPlan -Repository $fixture -Baseline $baseline -Current $candidate } 'Accepted an ignored compiled source change.'
    [IO.File]::WriteAllText((Join-Path $engineDir 'ignored_compiled.rs'), 'verified ignored source')
    $fakeGitlinkId = 'b' * 40
    $candidate = Get-VerificationInputSnapshot -Repository $fixture
    Assert-Rejected { Get-RoutineRonPlan -Repository $fixture -Baseline $baseline -Current $candidate } 'Accepted a changed committed Git submodule.'
    $fakeGitlinkId = 'a' * 40
    $fakeGitlinkDirty = $true
    Assert-Rejected { Get-VerificationInputSnapshot -Repository $fixture } 'Accepted a dirty Git submodule.'
    $fakeGitlinkDirty = $false

    $oldCard = Join-Path $data 'other.ron'
    Remove-Item -LiteralPath $oldCard
    $candidate = Get-VerificationInputSnapshot -Repository $fixture
    Assert-Rejected { Get-RoutineRonPlan -Repository $fixture -Baseline $baseline -Current $candidate } 'Accepted a deleted card definition.'
    [IO.File]::WriteAllText($oldCard, 'old card')

    $definition.tokens = @('shared token')
    $definition | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $map
    $candidate = Get-VerificationInputSnapshot -Repository $fixture
    Assert-Rejected { Get-RoutineRonPlan -Repository $fixture -Baseline $baseline -Current $candidate } 'Accepted a card with unvalidated shared-token dependencies.'
    $definition.tokens = @()
    $definition.semantic_fixtures = @(@{ test = 'batch_registry::example::definition'; covers = 'definition' })
    $definition | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $map
    $candidate = Get-VerificationInputSnapshot -Repository $fixture
    Assert-Rejected { Get-RoutineRonPlan -Repository $fixture -Baseline $baseline -Current $candidate } 'Accepted a card without an actual-card scenario.'
    $definition.semantic_fixtures = @(@{ test = 'scenario example::actual_card'; covers = 'actual behavior' })
    $definition.complete_definition_review_confirmed = $false
    $definition | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $map
    $candidate = Get-VerificationInputSnapshot -Repository $fixture
    Assert-Rejected { Get-RoutineRonPlan -Repository $fixture -Baseline $baseline -Current $candidate } 'Accepted unconfirmed semantic review.'
    $definition.complete_definition_review_confirmed = $true
    $definition.semantic_fixtures = @(@{ test = 'scenario new_module::actual_card'; covers = 'actual behavior' })
    $definition | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $map
    [IO.File]::WriteAllText((Join-Path $scenarios 'new_module.rs'), 'new test module')
    $candidate = Get-VerificationInputSnapshot -Repository $fixture
    Assert-Rejected { Get-RoutineRonPlan -Repository $fixture -Baseline $baseline -Current $candidate } 'Accepted a new test module.'
}
finally { Remove-WorkflowFixture $fixture }
Write-Output 'PASS routine RON eligibility and affected-test selection'
