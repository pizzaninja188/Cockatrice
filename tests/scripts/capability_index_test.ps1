param([string] $PowerShell = (Get-Process -Id $PID).Path)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$checker = Join-Path $repo 'scripts/check-capability-index.ps1'

function Assert-CapabilityTest {
    param([bool] $Condition, [string] $Message)
    if (-not $Condition) { throw $Message }
}

function Write-CapabilityText {
    param([string] $Path, [string] $Content)
    $parent = Split-Path -Parent $Path
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
    [IO.File]::WriteAllText($Path, $Content, [System.Text.UTF8Encoding]::new($false))
}

function Get-CapabilityEntryPath {
    param([string] $Root)
    Join-Path $Root 'tricerules/tricerules-cards/authoring/capabilities/entries/demo.md'
}

function Get-CapabilityEntryText {
    param([string] $Revision = '1111111111111111111111111111111111111111')
    $text = @'
# Demo artifact recovery

## Identity

- **Pattern ID:** `demo`
- **Category:** Activated ability
- **Search terms:** artifact recovery, demo, return

## Behavior and limits

- **Behavior:** Returns one artifact card from the graveyard to its owner's hand.
- **Composition and prerequisites:** The ability targets one artifact card in a graveyard.
- **Boundaries:** Covers one target and one card returned on resolution.
- **Near misses:** Returning multiple cards or a permanent to the battlefield is not covered.

## Runtime support

- **Status:** `Supported` for this one-card return effect.
- **Typed symbols:** `DemoEffect` in `tricerules/tricerules-cards/src/primitives/demo.rs`.
- **Implementation references:** Consumer: `tricerules/tricerules-cards/src/primitives/demo.rs#DemoEffect`; helper: `tricerules/tricerules-cards/src/primitives/demo_aux.rs#demo_aux_symbol`.
- **Evidence and limits:** The cited fixture asserts the exact card moved to its owner's hand.

## Generator recognition

- **Status:** `Unassessed`.
- **Recipe and input shapes:** Not reviewed.
- **References and limits:** Generator behavior is unassessed.

## Shipped card evidence

- `demo_card` __DASH__ definition: [Demo Card](../../../data/demo_card.ron) __DASH__ review map: [map](../../review-maps/demo_card.json).
- **Whole-card readiness:** `Unassessed` for `demo_card`.

## Semantic test coverage

- `scenario demo_target::returns_the_card` __DASH__ `tricerules/tricerules-core/tests/scenario/demo_target.rs#returns_the_card` __DASH__ `Exercised`: the independently expected card reaches its owner's hand.
- **Uncovered behavior:** Other card types and multiple-card returns.
- **Inapplicable cases:** `N/A` __DASH__ generator-recognition behavior is not exercised by this runtime scenario.

## Presentation prerequisites

- `None` __DASH__ this pattern has no player prompt or client-only presentation prerequisite.

## Review provenance

- **Reviewed revision:** `__REVISION__`.
- **Reviewed paths:**
  - `tricerules/tricerules-cards/data/demo_card.ron`
  - `tricerules/tricerules-cards/authoring/review-maps/demo_card.json`
  - `tricerules/tricerules-core/tests/scenario/demo_target.rs`
  - `tricerules/tricerules-cards/src/primitives/demo.rs`
  - `tricerules/tricerules-cards/src/primitives/demo_aux.rs`
- **Review note:** Isolated checker fixture; no semantic support claim is made for the repository.
'@
    return $text.Replace('__DASH__', [string][char]0x2014).Replace('__REVISION__', $Revision)
}

function New-CapabilityFixture {
    $root = Join-Path ([IO.Path]::GetTempPath()) ('cockatrice capability ' + [guid]::NewGuid())
    $entry = Get-CapabilityEntryPath $root
    Write-CapabilityText $entry (Get-CapabilityEntryText)
    Write-CapabilityText (Join-Path $root 'tricerules/tricerules-cards/authoring/capabilities/entries/ENTRY-TEMPLATE.md') @'
# Template placeholder

TODO placeholder with a deliberately invalid local target: [missing](not-present.md).
It must never be read as a catalogue entry.
'@
    Write-CapabilityText (Join-Path $root 'tricerules/tricerules-cards/data/demo_card.ron') @'
(
    id: "demo_card",
    name: "Demo Card",
)
'@
    Write-CapabilityText (Join-Path $root 'tricerules/tricerules-cards/authoring/review-maps/demo_card.json') @'
{
  "semantic_fixtures": [
    { "test": "scenario demo_target::returns_the_card", "covers": "the exact card returns to hand" }
  ]
}
'@
    Write-CapabilityText (Join-Path $root 'tricerules/tricerules-cards/src/primitives/demo.rs') @'
pub enum DemoEffect { ReturnOneCard }
'@
    Write-CapabilityText (Join-Path $root 'tricerules/tricerules-cards/src/primitives/demo_aux.rs') @'
pub fn demo_aux_symbol() {}
'@
    Write-CapabilityText (Join-Path $root 'tricerules/tricerules-core/tests/scenario/demo_target.rs') @'
#[test]
fn returns_the_card() {}
'@
    Write-CapabilityText (Join-Path $root 'unrelated.md') 'baseline unrelated file'
    return $root
}

function Invoke-CapabilityChecker {
    param([string] $Root, [switch] $CheckFreshness, [switch] $UseScriptDefault)
    $scriptPath = $checker
    $arguments = @('-NoProfile', '-File')
    if ($UseScriptDefault) {
        $scriptsDirectory = Join-Path $Root 'scripts'
        New-Item -ItemType Directory -Path $scriptsDirectory -Force | Out-Null
        $scriptPath = Join-Path $scriptsDirectory 'check-capability-index.ps1'
        Copy-Item -LiteralPath $checker -Destination $scriptPath
    }
    $arguments += $scriptPath
    if (-not $UseScriptDefault) { $arguments += @('-RepositoryRoot', $Root) }
    if ($CheckFreshness) { $arguments += '-CheckFreshness' }
    $savedPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $output = (& $PowerShell @arguments 2>&1 | Out-String)
        $code = $LASTEXITCODE
    }
    finally { $ErrorActionPreference = $savedPreference }
    return [pscustomobject]@{ ExitCode = $code; Output = $output }
}

function Remove-CapabilityFixture {
    param([string] $Root)
    $resolved = [IO.Path]::GetFullPath($Root)
    $tempParent = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if (-not $resolved.StartsWith($tempParent, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove unexpected fixture: $resolved"
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}

function Assert-CapabilityRejects {
    param([string] $Label, [scriptblock] $Mutation)
    $root = New-CapabilityFixture
    try {
        & $Mutation $root
        $result = Invoke-CapabilityChecker -Root $root
        Assert-CapabilityTest ($result.ExitCode -ne 0) "Checker accepted $Label. Output: $($result.Output)"
    }
    finally { Remove-CapabilityFixture $root }
}

function Invoke-FixtureGit {
    param([string] $Root, [string[]] $Arguments)
    $savedPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $output = & git -C $Root @Arguments 2>&1
        $exitCode = $LASTEXITCODE
    }
    finally { $ErrorActionPreference = $savedPreference }
    if ($exitCode -ne 0) { throw "Fixture git failed ($($Arguments -join ' ')): $($output -join "`n")" }
    return (($output | Out-String).Trim())
}

function New-FreshnessFixture {
    param([switch] $LeaveAuxiliaryUntracked)
    $root = New-CapabilityFixture
    try {
        $reviewedPaths = @(
            'tricerules/tricerules-cards/data/demo_card.ron',
            'tricerules/tricerules-cards/authoring/review-maps/demo_card.json',
            'tricerules/tricerules-core/tests/scenario/demo_target.rs',
            'tricerules/tricerules-cards/src/primitives/demo.rs',
            'tricerules/tricerules-cards/src/primitives/demo_aux.rs',
            'unrelated.md'
        )
        $null = Invoke-FixtureGit $root @('init', '--quiet')
        $pathsToCommit = if ($LeaveAuxiliaryUntracked) {
            @($reviewedPaths | Where-Object { $_ -ne 'tricerules/tricerules-cards/src/primitives/demo_aux.rs' })
        }
        else { $reviewedPaths }
        $null = Invoke-FixtureGit $root (@('add', '--') + $pathsToCommit)
        $null = Invoke-FixtureGit $root @('-c', 'user.name=Capability Fixture', '-c', 'user.email=capability-fixture@example.invalid', 'commit', '--quiet', '-m', 'fixture source baseline')
        $revision = Invoke-FixtureGit $root @('rev-parse', 'HEAD')
        $entryPath = Get-CapabilityEntryPath $root
        Write-CapabilityText $entryPath (Get-CapabilityEntryText -Revision $revision)
        return $root
    }
    catch {
        Remove-CapabilityFixture $root
        throw
    }
}

$fixture = New-CapabilityFixture
try {
    $result = Invoke-CapabilityChecker -Root $fixture
    Assert-CapabilityTest ($result.ExitCode -eq 0) "Valid fixture was rejected or template was included: $($result.Output)"
    Assert-CapabilityTest ($result.Output -notmatch 'ENTRY-TEMPLATE|template placeholder') 'Template leaked into catalogue output.'
}
finally { Remove-CapabilityFixture $fixture }

$defaultFixture = New-CapabilityFixture
try {
    $result = Invoke-CapabilityChecker -Root $defaultFixture -UseScriptDefault
    Assert-CapabilityTest ($result.ExitCode -eq 0) "Default repository-root resolution failed: $($result.Output)"
}
finally { Remove-CapabilityFixture $defaultFixture }

Assert-CapabilityRejects 'an empty catalogue' {
    param($root)
    Remove-Item -LiteralPath (Get-CapabilityEntryPath $root)
}
$duplicateFixture = New-CapabilityFixture
try {
    $copy = Get-CapabilityEntryText
    Write-CapabilityText (Join-Path $duplicateFixture 'tricerules/tricerules-cards/authoring/capabilities/entries/demo-copy.md') $copy
    $result = Invoke-CapabilityChecker -Root $duplicateFixture
    Assert-CapabilityTest ($result.ExitCode -ne 0 -and $result.Output -match '(?i)duplicate') `
        "Checker did not diagnose a duplicate pattern ID: $($result.Output)"
}
finally { Remove-CapabilityFixture $duplicateFixture }
Assert-CapabilityRejects 'a filename and pattern-ID mismatch' {
    param($root)
    Move-Item -LiteralPath (Get-CapabilityEntryPath $root) -Destination (Join-Path $root 'tricerules/tricerules-cards/authoring/capabilities/entries/not-demo.md')
}
Assert-CapabilityRejects 'a missing required heading' {
    param($root)
    $path = Get-CapabilityEntryPath $root
    $text = [IO.File]::ReadAllText($path).Replace('## Runtime support', '## Runtime evidence')
    Write-CapabilityText $path $text
}
Assert-CapabilityRejects 'a missing required field' {
    param($root)
    $path = Get-CapabilityEntryPath $root
    $text = [IO.File]::ReadAllText($path).Replace('- **Category:** Activated ability', '- **Category:**')
    Write-CapabilityText $path $text
}
Assert-CapabilityRejects 'a blank status or TODO placeholder' {
    param($root)
    $path = Get-CapabilityEntryPath $root
    $text = [IO.File]::ReadAllText($path).Replace('- **Status:** `Supported`', '- **Status:** ``')
    Write-CapabilityText $path $text
}
Assert-CapabilityRejects 'a TODO placeholder' {
    param($root)
    $path = Get-CapabilityEntryPath $root
    $text = [IO.File]::ReadAllText($path).Replace('Isolated checker fixture', 'TODO isolated checker fixture')
    Write-CapabilityText $path $text
}
Assert-CapabilityRejects 'a missing local Markdown target' {
    param($root)
    $path = Get-CapabilityEntryPath $root
    $text = [IO.File]::ReadAllText($path).Replace('../../../data/demo_card.ron', '../../../data/missing_card.ron')
    Write-CapabilityText $path $text
}
Assert-CapabilityRejects 'a missing implementation source anchor' {
    param($root)
    $path = Get-CapabilityEntryPath $root
    $text = [IO.File]::ReadAllText($path).Replace('#DemoEffect', '#MissingEffect')
    Write-CapabilityText $path $text
}
Assert-CapabilityRejects 'a card ID that disagrees with its linked definition' {
    param($root)
    $path = Join-Path $root 'tricerules/tricerules-cards/data/demo_card.ron'
    $text = [IO.File]::ReadAllText($path).Replace('id: "demo_card"', 'id: "different_card"')
    Write-CapabilityText $path $text
}
Assert-CapabilityRejects 'a semantic test absent from its linked review map' {
    param($root)
    $path = Join-Path $root 'tricerules/tricerules-cards/authoring/review-maps/demo_card.json'
    Write-CapabilityText $path '{ "semantic_fixtures": [] }'
}
Assert-CapabilityRejects 'a semantic test absent from its source file' {
    param($root)
    $path = Join-Path $root 'tricerules/tricerules-core/tests/scenario/demo_target.rs'
    Write-CapabilityText $path "#[test]`nfn another_test() {}"
}
Assert-CapabilityRejects 'a cited path omitted from Reviewed paths' {
    param($root)
    $path = Get-CapabilityEntryPath $root
    $text = [IO.File]::ReadAllText($path) -replace '(?m)^\s+- `tricerules/tricerules-cards/src/primitives/demo.rs`\r?\n', ''
    Write-CapabilityText $path $text
}
Assert-CapabilityRejects 'an invalid Reviewed path' {
    param($root)
    $path = Get-CapabilityEntryPath $root
    $text = [IO.File]::ReadAllText($path).Replace('tricerules/tricerules-cards/src/primitives/demo.rs`', 'tricerules/tricerules-cards/src/primitives/absent.rs`')
    Write-CapabilityText $path $text
}

$fresh = New-FreshnessFixture
try {
    $result = Invoke-CapabilityChecker -Root $fresh -CheckFreshness
    Assert-CapabilityTest ($result.ExitCode -eq 0) "Unchanged reviewed paths were not fresh: $($result.Output)"
    Assert-CapabilityTest ($result.Output -notmatch '(?i)warning.*fresh|fresh.*warning') "Unchanged paths produced a freshness warning: $($result.Output)"

    Write-CapabilityText (Join-Path $fresh 'unrelated.md') 'unrelated tracked edit'
    $result = Invoke-CapabilityChecker -Root $fresh -CheckFreshness
    Assert-CapabilityTest ($result.ExitCode -eq 0) "Unrelated change failed freshness check: $($result.Output)"
    Assert-CapabilityTest ($result.Output -notmatch '(?i)warning.*fresh|fresh.*warning') "Unrelated change invalidated entry freshness: $($result.Output)"

    Write-CapabilityText (Join-Path $fresh 'tricerules/tricerules-cards/src/primitives/demo.rs') 'pub enum DemoEffect { Changed }'
    $result = Invoke-CapabilityChecker -Root $fresh -CheckFreshness
    Assert-CapabilityTest ($result.Output -match '(?i)(warning|stale).{0,80}(fresh|reviewed|path)|(fresh|reviewed|path).{0,80}(warning|stale)') `
        "Relevant reviewed-path change did not warn: $($result.Output)"

    $entryPath = Get-CapabilityEntryPath $fresh
    $text = [IO.File]::ReadAllText($entryPath).Replace('1111111111111111111111111111111111111111', 'zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz')
    $text = $text -replace '(?m)(\*\*Reviewed revision:\*\* `)[^`]+(`\.)', '${1}zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz${2}'
    Write-CapabilityText $entryPath $text
    $result = Invoke-CapabilityChecker -Root $fresh -CheckFreshness
    $unableToAssess = $result.Output -match '(?i)(unable|cannot|could not).{0,80}(assess|resolve|revision|commit|git)'
    Assert-CapabilityTest (($result.ExitCode -ne 0) -or $unableToAssess) "Invalid reviewed revision was treated as fresh: $($result.Output)"

    $text = [IO.File]::ReadAllText($entryPath).Replace('zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz', '0000000000000000000000000000000000000000')
    Write-CapabilityText $entryPath $text
    $result = Invoke-CapabilityChecker -Root $fresh -CheckFreshness
    Assert-CapabilityTest ($result.ExitCode -eq 0 -and $result.Output -match '(?i)WARNING.*Unable to assess freshness.*does not resolve') `
        "Syntactically valid nonexistent revision was not reported as unable to assess: $($result.Output)"
}
finally { Remove-CapabilityFixture $fresh }

$untracked = New-FreshnessFixture -LeaveAuxiliaryUntracked
try {
    $result = Invoke-CapabilityChecker -Root $untracked -CheckFreshness
    Assert-CapabilityTest ($result.Output -match '(?i)(warning|stale).{0,80}(fresh|reviewed|path)|(fresh|reviewed|path).{0,80}(warning|stale)') `
        "Relevant untracked Reviewed path did not warn: $($result.Output)"
}
finally { Remove-CapabilityFixture $untracked }

Write-Output 'PASS capability index checker structure, evidence references, and scoped freshness regressions'
