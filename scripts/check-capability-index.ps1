<#
.SYNOPSIS
    Check the source references and basic structure of the capability pattern index.
.DESCRIPTION
    This standalone, source-only checker does not invoke Cargo, tests, generators, or network
    access. Optional freshness checks compare only each entry's Reviewed paths with its recorded
    Git revision. Lexical source checks are best effort and do not prove compilation or semantics.
#>
[CmdletBinding()]
param(
    [string] $RepositoryRoot,
    [switch] $CheckFreshness
)

$ErrorActionPreference = 'Stop'
$script:Errors = New-Object 'System.Collections.Generic.List[string]'
$script:Warnings = New-Object 'System.Collections.Generic.List[string]'

function Add-CheckError {
    param([string] $Entry, [string] $Message)
    $null = $script:Errors.Add("ERROR [$Entry] $Message")
}

function Add-CheckWarning {
    param([string] $Entry, [string] $Message)
    $null = $script:Warnings.Add("WARNING [$Entry] $Message")
}

function Test-UnderRepositoryRoot {
    param([string] $Path)
    if ($Path.Equals($script:Root.TrimEnd([char[]]@(92, 47)), [StringComparison]::OrdinalIgnoreCase)) { return $true }
    return $Path.StartsWith($script:RootPrefix, [StringComparison]::OrdinalIgnoreCase)
}

function ConvertTo-RepositoryPath {
    param([string] $FullPath)
    if (-not (Test-UnderRepositoryRoot $FullPath)) { return $null }
    return $FullPath.Substring($script:RootPrefix.Length).Replace('\', '/')
}

function Resolve-RepositoryFile {
    param([string] $Path, [string] $Entry, [string] $Role)
    if ([string]::IsNullOrWhiteSpace($Path)) {
        Add-CheckError $Entry "$Role is blank."
        return $null
    }
    if ($Path.Contains('\') -or [IO.Path]::IsPathRooted($Path) -or
        @($Path.Split('/') | Where-Object { $_ -in @('', '.', '..') }).Count -gt 0) {
        Add-CheckError $Entry "$Role must be a normalized repository-relative path using forward slashes: $Path"
        return $null
    }
    $fullPath = [IO.Path]::GetFullPath((Join-Path $script:Root ($Path -replace '/', [IO.Path]::DirectorySeparatorChar)))
    if (-not (Test-UnderRepositoryRoot $fullPath)) {
        Add-CheckError $Entry "$Role resolves outside the repository: $Path"
        return $null
    }
    if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
        Add-CheckError $Entry "$Role does not exist: $Path"
        return $null
    }
    return [pscustomobject]@{ FullPath = $fullPath; RepositoryPath = (ConvertTo-RepositoryPath $fullPath) }
}

function Resolve-MarkdownTarget {
    param([string] $Target, [string] $Entry, [string] $EntryDirectory)
    $Target = $Target.Trim()
    if ($Target -match '^(?i)(https?|mailto):') { return $null }
    if ($Target.StartsWith('#')) { return $null }
    if ($Target.StartsWith('<') -and $Target.EndsWith('>')) {
        $Target = $Target.Substring(1, $Target.Length - 2)
    }
    $pathPart = ($Target -split '[#?]', 2)[0]
    if ([string]::IsNullOrWhiteSpace($pathPart)) { return $null }
    if ($pathPart.Contains('\') -or [IO.Path]::IsPathRooted($pathPart) -or $pathPart -match '^[A-Za-z]:') {
        Add-CheckError $Entry "Local Markdown target must be relative to the entry: $Target"
        return $null
    }
    try {
        $decodedPath = [Uri]::UnescapeDataString($pathPart)
        $fullPath = [IO.Path]::GetFullPath((Join-Path $EntryDirectory ($decodedPath -replace '/', [IO.Path]::DirectorySeparatorChar)))
    }
    catch {
        Add-CheckError $Entry "Could not resolve Markdown target '$Target': $($_.Exception.Message)"
        return $null
    }
    if (-not (Test-UnderRepositoryRoot $fullPath)) {
        Add-CheckError $Entry "Markdown target resolves outside the repository: $Target"
        return $null
    }
    if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
        Add-CheckError $Entry "Markdown target does not exist: $Target"
        return $null
    }
    return [pscustomobject]@{ FullPath = $fullPath; RepositoryPath = (ConvertTo-RepositoryPath $fullPath) }
}

function Get-SectionText {
    param([string] $Content, [string] $Heading)
    $pattern = '(?ms)^##[ \t]+' + [regex]::Escape($Heading) + '[ \t]*\r?\n(?<body>.*?)(?=^##[ \t]|\z)'
    $match = [regex]::Match($Content, $pattern)
    if (-not $match.Success) { return $null }
    return $match.Groups['body'].Value
}

function Get-RequiredField {
    param([string] $Section, [string] $Field, [string] $Entry)
    $pattern = '(?m)^\s*-\s+\*\*' + [regex]::Escape($Field) + ':\*\*[ \t]*(?<value>[^\r\n]*)'
    $matches = [regex]::Matches([string] $Section, $pattern)
    if ($matches.Count -eq 0) {
        Add-CheckError $Entry "Missing required field '$Field'."
        return $null
    }
    if ($matches.Count -gt 1) { Add-CheckError $Entry "Field '$Field' appears more than once." }
    $value = $matches[0].Groups['value'].Value.Trim()
    if ([string]::IsNullOrWhiteSpace($value)) {
        Add-CheckError $Entry "Field '$Field' is blank."
        return $null
    }
    return $value
}

function Test-StatusField {
    param([string] $Value, [string[]] $Allowed, [string] $Field, [string] $Entry)
    if ([string]::IsNullOrWhiteSpace($Value)) { return }
    $match = [regex]::Match($Value, '`(?<status>[^`]+)`')
    if (-not $match.Success) {
        Add-CheckError $Entry "$Field must name a nonblank status in inline code."
        return
    }
    if ($Allowed -cnotcontains $match.Groups['status'].Value) {
        Add-CheckError $Entry "$Field has unknown status '$($match.Groups['status'].Value)'."
    }
}

function Add-CitedPath {
    param([hashtable] $Cited, [string] $RepositoryPath)
    if (-not [string]::IsNullOrWhiteSpace($RepositoryPath)) { $Cited[$RepositoryPath] = $true }
}

function Get-ReviewedPaths {
    param([string] $ReviewSection, [string] $Entry)
    $block = [regex]::Match([string] $ReviewSection, '(?ms)^\s*-\s+\*\*Reviewed paths:\*\*[ \t]*\r?\n(?<paths>.*?)(?=^\s*-\s+\*\*Review note:\*\*|\z)')
    if (-not $block.Success) {
        Add-CheckError $Entry "Missing or blank 'Reviewed paths' list."
        return @()
    }
    $paths = @([regex]::Matches($block.Groups['paths'].Value, '(?m)^\s*-\s+`(?<path>[^`]+)`[ \t]*$') |
        ForEach-Object { $_.Groups['path'].Value.Trim() })
    if ($paths.Count -eq 0) { Add-CheckError $Entry "'Reviewed paths' must list at least one path." }
    return $paths
}

function Invoke-GitReadOnly {
    param([string[]] $Arguments)
    if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
        return [pscustomobject]@{ ExitCode = 127; Output = 'git executable is unavailable' }
    }
    $savedPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $output = @(& git -C $script:Root @Arguments 2>&1)
        $exitCode = $LASTEXITCODE
    }
    catch {
        $exitCode = 127
        $output = @($_.Exception.Message)
    }
    finally { $ErrorActionPreference = $savedPreference }
    return [pscustomobject]@{ ExitCode = $exitCode; Output = (($output | Out-String).Trim()) }
}

function Test-EntryFreshness {
    param([string] $Entry, [string] $Revision, [string[]] $ReviewedPaths)
    if ($Revision -notmatch '^(?:[0-9a-fA-F]{40}|[0-9a-fA-F]{64})$') {
        Add-CheckWarning $Entry "Unable to assess freshness: reviewed revision is not a full commit ID ('$Revision')."
        return
    }
    $resolved = Invoke-GitReadOnly @('rev-parse', '--verify', ($Revision + '^{commit}'))
    if ($resolved.ExitCode -ne 0 -or [string]::IsNullOrWhiteSpace($resolved.Output)) {
        Add-CheckWarning $Entry "Unable to assess freshness: reviewed revision '$Revision' does not resolve to a commit."
        return
    }
    $commit = ($resolved.Output -split '\r?\n')[0].Trim()
    $diffArgs = @('diff', '--name-only', '--no-renames', $commit, '--') + $ReviewedPaths
    $diff = Invoke-GitReadOnly $diffArgs
    $trackedArgs = @('ls-files', '--full-name', '--') + $ReviewedPaths
    $tracked = Invoke-GitReadOnly $trackedArgs
    if ($diff.ExitCode -ne 0 -or $tracked.ExitCode -ne 0) {
        Add-CheckWarning $Entry "Unable to assess freshness for reviewed paths: $($diff.Output) $($tracked.Output)"
        return
    }
    $changedSet = @{}
    foreach ($path in ($diff.Output -split '\r?\n')) {
        if (-not [string]::IsNullOrWhiteSpace($path)) { $changedSet[$path.Replace('\', '/')] = $true }
    }
    $trackedSet = @{}
    foreach ($path in ($tracked.Output -split '\r?\n')) {
        if (-not [string]::IsNullOrWhiteSpace($path)) { $trackedSet[$path.Replace('\', '/')] = $true }
    }
    foreach ($path in $ReviewedPaths) {
        if ($changedSet.ContainsKey($path)) {
            Add-CheckWarning $Entry "Reviewed path changed since $commit`: $path"
        }
        elseif (-not $trackedSet.ContainsKey($path)) {
            Add-CheckWarning $Entry "Reviewed path is untracked in the current worktree: $path"
        }
    }
}

try {
    if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
        if ([string]::IsNullOrWhiteSpace($PSScriptRoot)) { throw 'Could not determine the script directory for the default repository root.' }
        $RepositoryRoot = Split-Path -Parent $PSScriptRoot
    }
    $script:Root = [IO.Path]::GetFullPath($RepositoryRoot)
    $script:RootPrefix = $script:Root.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    if (-not (Test-Path -LiteralPath $script:Root -PathType Container)) {
        throw "Repository root does not exist: $script:Root"
    }
    $entriesDirectory = Join-Path $script:Root 'tricerules/tricerules-cards/authoring/capabilities/entries'
    if (-not (Test-Path -LiteralPath $entriesDirectory -PathType Container)) {
        Add-CheckError 'catalogue' 'Missing entries directory: tricerules/tricerules-cards/authoring/capabilities/entries'
        $entries = @()
    }
    else {
        $entries = @(Get-ChildItem -LiteralPath $entriesDirectory -Filter '*.md' -File |
            Where-Object { $_.Name -cne 'ENTRY-TEMPLATE.md' } | Sort-Object Name)
    }
    if ($entries.Count -eq 0) { Add-CheckError 'catalogue' 'No capability entries found (ENTRY-TEMPLATE.md is excluded).' }

    $seenIds = @{}
    $freshnessRecords = New-Object 'System.Collections.Generic.List[object]'
    $requiredHeadings = @(
        'Identity', 'Behavior and limits', 'Runtime support', 'Generator recognition',
        'Shipped card evidence', 'Semantic test coverage', 'Presentation prerequisites', 'Review provenance'
    )

    foreach ($entryFile in $entries) {
        $entryName = $entryFile.Name
        $content = [IO.File]::ReadAllText($entryFile.FullName)
        if ($content -match '(?i)\bTODO\b') { Add-CheckError $entryName 'Entry still contains a TODO placeholder.' }
        if ($content -notmatch '(?m)^#[ \t]+\S') { Add-CheckError $entryName 'Missing nonblank level-one title.' }

        $sections = @{}
        foreach ($heading in $requiredHeadings) {
            $sections[$heading] = Get-SectionText $content $heading
            if ($null -eq $sections[$heading]) { Add-CheckError $entryName "Missing required heading '## $heading'." }
        }
        $patternIdField = Get-RequiredField $sections['Identity'] 'Pattern ID' $entryName
        $category = Get-RequiredField $sections['Identity'] 'Category' $entryName
        $searchTerms = Get-RequiredField $sections['Identity'] 'Search terms' $entryName
        $patternIdMatch = [regex]::Match([string] $patternIdField, '`(?<id>[^`]+)`')
        $patternId = if ($patternIdMatch.Success) { $patternIdMatch.Groups['id'].Value } else { '' }
        if (-not $patternIdMatch.Success -or $patternId -notmatch '^[a-z0-9]+(?:-[a-z0-9]+)*$') {
            Add-CheckError $entryName 'Pattern ID must be a lowercase kebab-case ID in inline code.'
        }
        $fileId = [IO.Path]::GetFileNameWithoutExtension($entryFile.Name)
        if ($patternId -and $patternId -cne $fileId) {
            Add-CheckError $entryName "Filename must match Pattern ID '$patternId'."
        }
        if ($patternId) {
            if ($seenIds.ContainsKey($patternId)) { Add-CheckError $entryName "Duplicate Pattern ID '$patternId'." }
            else { $seenIds[$patternId] = $entryName }
        }

        foreach ($field in @('Behavior', 'Composition and prerequisites', 'Boundaries', 'Near misses')) {
            $null = Get-RequiredField $sections['Behavior and limits'] $field $entryName
        }
        $runtimeStatus = Get-RequiredField $sections['Runtime support'] 'Status' $entryName
        Test-StatusField $runtimeStatus @('Supported', 'Partial', 'Unsupported', 'Unassessed') 'Runtime support status' $entryName
        foreach ($field in @('Typed symbols', 'Implementation references', 'Evidence and limits')) {
            $null = Get-RequiredField $sections['Runtime support'] $field $entryName
        }
        $generatorStatus = Get-RequiredField $sections['Generator recognition'] 'Status' $entryName
        Test-StatusField $generatorStatus @('Recognized', 'Partial', 'Not recognized', 'Unassessed') 'Generator recognition status' $entryName
        foreach ($field in @('Recipe and input shapes', 'References and limits')) {
            $null = Get-RequiredField $sections['Generator recognition'] $field $entryName
        }
        $wholeReadiness = Get-RequiredField $sections['Shipped card evidence'] 'Whole-card readiness' $entryName
        Test-StatusField $wholeReadiness @('Ready', 'Partial', 'Unassessed') 'Whole-card readiness' $entryName
        foreach ($field in @('Uncovered behavior', 'Inapplicable cases')) {
            $null = Get-RequiredField $sections['Semantic test coverage'] $field $entryName
        }
        if ([string]::IsNullOrWhiteSpace($sections['Presentation prerequisites']) -or
            [string]::IsNullOrWhiteSpace($sections['Presentation prerequisites'].Trim())) {
            Add-CheckError $entryName "'Presentation prerequisites' must state requirements or a reasoned None."
        }
        $revisionValue = Get-RequiredField $sections['Review provenance'] 'Reviewed revision' $entryName
        $revisionMatch = [regex]::Match([string] $revisionValue, '`(?<revision>[^`]+)`')
        $revision = if ($revisionMatch.Success) { $revisionMatch.Groups['revision'].Value } else { '' }
        if ($revision -notmatch '^(?:[0-9a-fA-F]{40}|[0-9a-fA-F]{64})$') {
            Add-CheckError $entryName 'Reviewed revision must be a full 40- or 64-character hexadecimal commit ID.'
        }
        foreach ($field in @('Review note')) { $null = Get-RequiredField $sections['Review provenance'] $field $entryName }

        $citedPaths = @{}
        $reviewedPaths = @()
        $reviewedPaths = Get-ReviewedPaths $sections['Review provenance'] $entryName
        $reviewedSet = @{}
        $validReviewedPaths = New-Object 'System.Collections.Generic.List[string]'
        foreach ($path in $reviewedPaths) {
            $resolvedPath = Resolve-RepositoryFile $path $entryName 'Reviewed path'
            if ($null -ne $resolvedPath) {
                if ($reviewedSet.ContainsKey($resolvedPath.RepositoryPath)) {
                    Add-CheckError $entryName "Reviewed path is listed more than once: $path"
                }
                $reviewedSet[$resolvedPath.RepositoryPath] = $true
                $null = $validReviewedPaths.Add($resolvedPath.RepositoryPath)
            }
        }

        $links = [regex]::Matches($content, '!?(?:\[[^\]]*\])\((?<target>[^)]+)\)')
        foreach ($link in $links) {
            $target = $link.Groups['target'].Value
            $resolvedTarget = Resolve-MarkdownTarget $target $entryName $entryFile.DirectoryName
            if ($null -ne $resolvedTarget) { Add-CitedPath $citedPaths $resolvedTarget.RepositoryPath }
        }

        foreach ($inline in [regex]::Matches($content, '`(?<value>[^`\r\n]+)`')) {
            $value = $inline.Groups['value'].Value.Trim()
            if ($value.Contains('#')) {
                $separator = $value.IndexOf('#')
                $sourcePath = $value.Substring(0, $separator)
                $symbol = $value.Substring($separator + 1)
                if ([string]::IsNullOrWhiteSpace($sourcePath) -or [string]::IsNullOrWhiteSpace($symbol)) {
                    Add-CheckError $entryName "Malformed source anchor: $value"
                    continue
                }
                $resolvedSource = Resolve-RepositoryFile $sourcePath $entryName 'Source anchor path'
                if ($null -ne $resolvedSource) {
                    Add-CitedPath $citedPaths $resolvedSource.RepositoryPath
                    $sourceText = [IO.File]::ReadAllText($resolvedSource.FullPath)
                    $symbolName = ($symbol -split '::')[-1]
                    if ($sourceText -cnotmatch ('(?<![A-Za-z0-9_])' + [regex]::Escape($symbolName) + '(?![A-Za-z0-9_])')) {
                        Add-CheckError $entryName "Source anchor symbol '$symbol' is not present in $sourcePath."
                    }
                }
            }
            elseif ($value -match '^(?<path>(?:[A-Za-z0-9_.-]+/)+[A-Za-z0-9_.-]+\.(?:rs|ron|json|md|cpp|h|hpp|qml|ui|svg|png|qrc))$') {
                $resolvedSource = Resolve-RepositoryFile $Matches['path'] $entryName 'Cited source path'
                if ($null -ne $resolvedSource) { Add-CitedPath $citedPaths $resolvedSource.RepositoryPath }
            }
        }

        $shippedSection = [string] $sections['Shipped card evidence']
        $cardPattern = '(?ms)^\s*-\s+`(?<id>[a-z0-9_]+)`\s+\u2014\s+definition:\s*\[(?<name>[^\]]+)\]\((?<definition>[^)]+)\)\s+\u2014\s+review\s+map:\s*\[[^\]]+\]\((?<map>[^)]+)\)'
        $cardRecords = [regex]::Matches($shippedSection, $cardPattern)
        if ($cardRecords.Count -eq 0) { Add-CheckError $entryName 'No shipped card ID with linked definition and review map was found.' }
        $linkedMapTests = New-Object 'System.Collections.Generic.List[string]'
        foreach ($card in $cardRecords) {
            $cardId = $card.Groups['id'].Value
            $definition = Resolve-MarkdownTarget $card.Groups['definition'].Value $entryName $entryFile.DirectoryName
            $map = Resolve-MarkdownTarget $card.Groups['map'].Value $entryName $entryFile.DirectoryName
            if ($null -ne $definition) {
                Add-CitedPath $citedPaths $definition.RepositoryPath
                $ronText = [IO.File]::ReadAllText($definition.FullPath)
                $idMatch = [regex]::Match($ronText, '(?m)^\s*id:\s*"(?<id>[^"]+)"')
                if (-not $idMatch.Success) {
                    Add-CheckError $entryName "Could not find a literal RON id in linked definition for '$cardId'."
                }
                elseif ($idMatch.Groups['id'].Value -cne $cardId) {
                    Add-CheckError $entryName "Card ID '$cardId' disagrees with linked RON id '$($idMatch.Groups['id'].Value)'."
                }
                $nameMatch = [regex]::Match($ronText, '(?m)^\s*name:\s*"(?<name>[^"]+)"')
                if ($nameMatch.Success -and $nameMatch.Groups['name'].Value -cne $card.Groups['name'].Value) {
                    Add-CheckError $entryName "Linked card name '$($card.Groups['name'].Value)' disagrees with RON name '$($nameMatch.Groups['name'].Value)'."
                }
            }
            if ($null -ne $map) {
                Add-CitedPath $citedPaths $map.RepositoryPath
                $mapStem = [IO.Path]::GetFileNameWithoutExtension($map.FullPath)
                if ($mapStem -cne $cardId) { Add-CheckError $entryName "Review map filename '$mapStem' does not match card ID '$cardId'." }
                try {
                    $mapObject = [IO.File]::ReadAllText($map.FullPath) | ConvertFrom-Json
                    foreach ($fixture in @($mapObject.semantic_fixtures)) {
                        if (-not [string]::IsNullOrWhiteSpace([string] $fixture.test)) {
                            $null = $linkedMapTests.Add([string] $fixture.test)
                        }
                    }
                }
                catch { Add-CheckError $entryName "Linked review map is not valid JSON: $($map.RepositoryPath)" }
            }
        }

        $semanticSection = [string] $sections['Semantic test coverage']
        $testPattern = '(?ms)^\s*-\s+`(?<identity>[^`]+)`\s*\u2014\s*`(?<anchor>[^`]+\.rs#[^`]+)`\s*\u2014\s*`?(?<coverage>Exercised|N/A|Fixture-blocked)\b'
        $testRecords = [regex]::Matches($semanticSection, $testPattern)
        if ($testRecords.Count -eq 0) { Add-CheckError $entryName 'No parseable exact semantic test ID and Rust file/function anchor found.' }
        foreach ($test in $testRecords) {
            $identity = $test.Groups['identity'].Value.Trim()
            $anchor = $test.Groups['anchor'].Value.Trim()
            $testIdentityRemainder = $identity -replace '^scenario\s+', ''
            if ($identity -cnotmatch '^scenario\s+[A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)+$' -and
                $identity -cnotmatch '^[A-Za-z0-9_]+::[A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)*$') {
                Add-CheckError $entryName "Unsupported semantic test identity '$identity'."
            }
            if (-not ($linkedMapTests.ToArray() -ccontains $identity)) {
                Add-CheckError $entryName "Semantic test '$identity' is absent from every linked review map."
            }
            $anchorParts = $anchor.Split('#', 2)
            $resolvedTest = Resolve-RepositoryFile $anchorParts[0] $entryName 'Semantic test source path'
            if ($null -ne $resolvedTest) {
                Add-CitedPath $citedPaths $resolvedTest.RepositoryPath
                $testName = ($anchorParts[1] -split '::')[-1]
                $identityTestName = ($testIdentityRemainder -split '::')[-1]
                if ($testName -cne $identityTestName) {
                    Add-CheckError $entryName "Test identity '$identity' does not match source anchor function '$testName'."
                }
                $testSource = [IO.File]::ReadAllText($resolvedTest.FullPath)
                $functionPattern = '(?m)\bfn\s+' + [regex]::Escape($testName) + '\b'
                if ($testSource -cnotmatch $functionPattern) {
                    Add-CheckError $entryName "Semantic test function '$testName' is absent from $($resolvedTest.RepositoryPath)."
                }
            }
        }

        foreach ($path in $citedPaths.Keys) {
            if (-not $reviewedSet.ContainsKey($path)) {
                Add-CheckError $entryName "Cited path is missing from Reviewed paths: $path"
            }
        }
        if ($CheckFreshness -and $validReviewedPaths.Count -gt 0) {
            $null = $freshnessRecords.Add([pscustomobject]@{
                Entry = $entryName
                Revision = $revision
                Paths = @($validReviewedPaths.ToArray())
            })
        }
    }

    if ($CheckFreshness -and $script:Errors.Count -eq 0) {
        foreach ($record in $freshnessRecords) {
            Test-EntryFreshness $record.Entry $record.Revision $record.Paths
        }
    }

    foreach ($message in $script:Errors) { Write-Output $message }
    foreach ($message in $script:Warnings) { Write-Output $message }
    if ($script:Errors.Count -gt 0) {
        Write-Output "FAIL capability index: $($script:Errors.Count) error(s), $($script:Warnings.Count) warning(s)."
        exit 1
    }
    Write-Output "PASS capability index: $($entries.Count) entry file(s), $($script:Warnings.Count) warning(s)."
    exit 0
}
catch {
    Write-Output "ERROR [checker] $($_.Exception.Message)"
    exit 1
}
