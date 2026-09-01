$ErrorActionPreference = 'Stop'

function Assert-Equal {
    param(
        [Parameter(Mandatory)]
        $Expected,

        [Parameter(Mandatory)]
        $Actual,

        [Parameter(Mandatory)]
        [string] $Context
    )

    if ($Expected -ne $Actual) {
        throw "$Context expected '$Expected', found '$Actual'."
    }
}

function Get-MarkdownTableRows {
    param(
        [Parameter(Mandatory)]
        [AllowEmptyString()]
        [string[]] $Lines,

        [Parameter(Mandatory)]
        [string] $Marker
    )

    $markerIndex = [array]::IndexOf($Lines, $Marker)
    if ($markerIndex -lt 0) {
        throw "Markdown marker not found: $Marker"
    }

    $rows = @()
    $tableStarted = $false
    for ($index = $markerIndex + 1; $index -lt $Lines.Count; $index++) {
        $line = $Lines[$index]
        if (-not $line.StartsWith('|')) {
            if ($tableStarted) {
                break
            }
            continue
        }

        $tableStarted = $true
        $cells = @($line.Trim('|').Split('|') | ForEach-Object { $_.Trim().Trim('*') })
        if ($cells.Count -eq 0 -or $cells[0] -match '^[-:]+$') {
            continue
        }
        $rows += ,$cells
    }

    if ($rows.Count -lt 2) {
        throw "Markdown table after '$Marker' is missing data rows."
    }

    return $rows[1..($rows.Count - 1)]
}

function Format-Percentage {
    param(
        [Parameter(Mandatory)]
        [int] $Count,

        [Parameter(Mandatory)]
        [int] $Total
    )

    $percentage = [math]::Round(
        100.0 * $Count / $Total,
        1,
        [MidpointRounding]::AwayFromZero
    )
    return $percentage.ToString('0.0', [Globalization.CultureInfo]::InvariantCulture) + '%'
}

function Test-Calibration {
    param(
        [Parameter(Mandatory)]
        [string] $CsvPath,

        [Parameter(Mandatory)]
        [string] $MarkdownPath,

        [bool] $RequireGapTraceability = $false,

        [bool] $RequireCapabilityStats = $false,

        [int] $ExpectedRows = 180,

        [int] $ExpectedStrata = 3,

        [int] $RowsPerStratum = 60
    )

    $displayName = Split-Path -Leaf $CsvPath
    $rows = @(Import-Csv -LiteralPath $CsvPath)
    Assert-Equal $ExpectedRows $rows.Count "$displayName row count"

    $requiredColumns = @(
        'sample_id',
        'stratum',
        'selection_hash',
        'oracle_id',
        'name',
        'classification'
    )
    $columns = @($rows[0].PSObject.Properties.Name)
    foreach ($column in $requiredColumns) {
        if ($columns -notcontains $column) {
            throw "$displayName is missing required column '$column'."
        }
    }

    foreach ($identityColumn in 'sample_id', 'selection_hash', 'oracle_id', 'name') {
        $values = @($rows | ForEach-Object { $_.$identityColumn })
        Assert-Equal $rows.Count @($values | Sort-Object -Unique).Count "$displayName unique $identityColumn count"
        if ($values -contains '') {
            throw "$displayName contains an empty $identityColumn."
        }
    }
    foreach ($row in $rows) {
        if ($row.selection_hash -notmatch '^[0-9a-f]{64}$') {
            throw "$displayName row $($row.sample_id) has invalid selection hash '$($row.selection_hash)'."
        }
    }

    $knownClassifications = @('full_now', 'partial_now', 'blocked')
    $unknownClassifications = @(
        $rows.classification |
            Where-Object { $knownClassifications -notcontains $_ } |
            Sort-Object -Unique
    )
    Assert-Equal 0 $unknownClassifications.Count "$displayName unknown classification count"

    $strata = @($rows | Group-Object stratum | Sort-Object Name)
    Assert-Equal $ExpectedStrata $strata.Count "$displayName stratum count"
    foreach ($stratum in $strata) {
        Assert-Equal $RowsPerStratum $stratum.Count "$displayName $($stratum.Name) row count"
    }

    $full = @($rows | Where-Object classification -eq 'full_now').Count
    $partial = @($rows | Where-Object classification -eq 'partial_now').Count
    $blocked = @($rows | Where-Object classification -eq 'blocked').Count
    $fullOrPartial = $full + $partial

    $markdownLines = @(Get-Content -LiteralPath $MarkdownPath)
    $resultRows = Get-MarkdownTableRows -Lines $markdownLines -Marker '## Result'
    $resultByLabel = @{}
    foreach ($row in $resultRows) {
        $resultByLabel[$row[0]] = $row
    }

    $expectedResults = [ordered]@{
        'Fully implementable now' = @($full, (Format-Percentage -Count $full -Total $rows.Count))
        'Partially implementable now' = @($partial, (Format-Percentage -Count $partial -Total $rows.Count))
        'Full or partial' = @($fullOrPartial, (Format-Percentage -Count $fullOrPartial -Total $rows.Count))
        'Blocked' = @($blocked, (Format-Percentage -Count $blocked -Total $rows.Count))
        'Total' = @($rows.Count, '100.0%')
    }
    foreach ($label in $expectedResults.Keys) {
        if (-not $resultByLabel.ContainsKey($label)) {
            throw "$displayName Markdown result table is missing '$label'."
        }
        Assert-Equal $expectedResults[$label][0].ToString() $resultByLabel[$label][1] "$displayName result '$label' count"
        Assert-Equal $expectedResults[$label][1] $resultByLabel[$label][2] "$displayName result '$label' rate"
    }

    $stratumRows = Get-MarkdownTableRows -Lines $markdownLines -Marker 'By stratum:'
    $stratumByCode = @{}
    foreach ($row in $stratumRows) {
        $code = $row[0]
        if ($code -match '\(([^)]+)\)$') {
            $code = $Matches[1]
        }
        $stratumByCode[$code] = $row
    }

    foreach ($stratum in $strata) {
        if (-not $stratumByCode.ContainsKey($stratum.Name)) {
            throw "$displayName Markdown stratum table is missing '$($stratum.Name)'."
        }
        $row = $stratumByCode[$stratum.Name]
        $stratumFull = @($stratum.Group | Where-Object classification -eq 'full_now').Count
        $stratumPartial = @($stratum.Group | Where-Object classification -eq 'partial_now').Count
        $stratumBlocked = @($stratum.Group | Where-Object classification -eq 'blocked').Count
        Assert-Equal $stratumFull.ToString() $row[1] "$displayName $($stratum.Name) full count"
        Assert-Equal $stratumPartial.ToString() $row[2] "$displayName $($stratum.Name) partial count"
        Assert-Equal $stratumBlocked.ToString() $row[3] "$displayName $($stratum.Name) blocked count"
        Assert-Equal $stratum.Count.ToString() $row[4] "$displayName $($stratum.Name) total count"
    }

    if ($RequireGapTraceability) {
        foreach ($column in 'primary_gap', 'secondary_gaps') {
            if ($columns -notcontains $column) {
                throw "$displayName is missing required gap column '$column'."
            }
        }

        $fullRowsWithGaps = @(
            $rows | Where-Object {
                $_.classification -eq 'full_now' -and ($_.primary_gap -or $_.secondary_gaps)
            }
        )
        $nonFullRowsWithoutPrimaryGap = @(
            $rows | Where-Object {
                $_.classification -ne 'full_now' -and -not $_.primary_gap
            }
        )
        Assert-Equal 0 $fullRowsWithGaps.Count "$displayName full rows with gap labels"
        Assert-Equal 0 $nonFullRowsWithoutPrimaryGap.Count "$displayName non-full rows without primary gap"

        $observedGaps = @(
            $rows |
                Where-Object classification -ne 'full_now' |
                ForEach-Object {
                    if ($_.primary_gap) { $_.primary_gap }
                    @($_.secondary_gaps -split ';') | Where-Object { $_ }
                } |
                Sort-Object -Unique
        )
        $traceabilityRows = Get-MarkdownTableRows `
            -Lines $markdownLines `
            -Marker 'Full raw-label traceability:'
        $gapToIssue = @{}
        $mappedGaps = @(
            $traceabilityRows |
                ForEach-Object {
                    if ($_[0] -notmatch '#(\d+)') {
                        throw "$displayName traceability row has no issue number: '$($_[0])'."
                    }
                    $issueNumber = [int]$Matches[1]
                    [regex]::Matches($_[1], '`([^`]+)`') |
                        ForEach-Object {
                            $gap = $_.Groups[1].Value
                            if ($gapToIssue.ContainsKey($gap)) {
                                throw "$displayName maps gap '$gap' more than once."
                            }
                            $gapToIssue[$gap] = $issueNumber
                            $gap
                        }
                } |
                Sort-Object -Unique
        )

        $missingGaps = @($observedGaps | Where-Object { $mappedGaps -notcontains $_ })
        $extraGaps = @($mappedGaps | Where-Object { $observedGaps -notcontains $_ })
        Assert-Equal 0 $missingGaps.Count "$displayName unmapped observed gap count ($($missingGaps -join ', '))"
        Assert-Equal 0 $extraGaps.Count "$displayName stale mapped gap count ($($extraGaps -join ', '))"

        if ($RequireCapabilityStats) {
            $expectedStats = @{}
            foreach ($row in ($rows | Where-Object classification -ne 'full_now')) {
                $rowGaps = @($row.primary_gap) + @($row.secondary_gaps -split ';' | Where-Object { $_ })
                $rowIssues = @($rowGaps | ForEach-Object { $gapToIssue[$_] } | Sort-Object -Unique)
                foreach ($issueNumber in $rowIssues) {
                    if (-not $expectedStats.ContainsKey($issueNumber)) {
                        $expectedStats[$issueNumber] = [pscustomobject]@{
                            Occurrences = 0
                            SoleUnlocks = 0
                        }
                    }
                    $expectedStats[$issueNumber].Occurrences++
                }
                if ($rowIssues.Count -eq 1) {
                    $expectedStats[$rowIssues[0]].SoleUnlocks++
                }
            }

            $capabilityRows = Get-MarkdownTableRows `
                -Lines $markdownLines `
                -Marker 'The normalized capability gaps are:'
            $reportedIssues = [System.Collections.Generic.HashSet[int]]::new()
            foreach ($capabilityRow in $capabilityRows) {
                if ($capabilityRow[3] -notmatch '/issues/(\d+)') {
                    throw "$displayName capability row has no tracker issue: '$($capabilityRow[3])'."
                }
                $issueNumber = [int]$Matches[1]
                [void]$reportedIssues.Add($issueNumber)
                if (-not $expectedStats.ContainsKey($issueNumber)) {
                    throw "$displayName reports issue #$issueNumber with no mapped gaps."
                }
                Assert-Equal $expectedStats[$issueNumber].Occurrences.ToString() $capabilityRow[1] "$displayName issue #$issueNumber occurrence count"
                Assert-Equal $expectedStats[$issueNumber].SoleUnlocks.ToString() $capabilityRow[2] "$displayName issue #$issueNumber sole-gap unlock count"
            }
            foreach ($issueNumber in $expectedStats.Keys) {
                if (-not $reportedIssues.Contains([int]$issueNumber)) {
                    throw "$displayName capability table omits mapped issue #$issueNumber."
                }
            }
        }
    }
}

function Test-ExpandedCalibration {
    param([string] $Repo, [string] $Stem)

    $docs = Join-Path $Repo 'docs'
    $rows = @(Import-Csv (Join-Path $docs "$Stem.csv"))
    $population = @(Import-Csv (Join-Path $docs "$Stem-population.csv"))
    $gaps = @(Import-Csv (Join-Path $docs "$Stem-gaps.csv"))
    $manifest = Get-Content (Join-Path $docs "$Stem-manifest.json") -Raw | ConvertFrom-Json
    $rulingsData = Get-Content (Join-Path $docs "$Stem-rulings.json") -Raw -Encoding utf8 | ConvertFrom-Json
    $rulings = @($rulingsData)
    $markdown = @(Get-Content (Join-Path $docs "$Stem.md"))

    Assert-Equal 4 $manifest.version "$Stem manifest version"
    Assert-Equal $manifest.sample_size $rows.Count "$Stem manifest sample size"
    Assert-Equal 12 @($manifest.strata).Count "$Stem manifest strata"
    Assert-Equal 1457 $manifest.registry_name_count "$Stem registry baseline"
    Assert-Equal 13 @($manifest.new_issue_numbers).Count "$Stem new issue count"
    Assert-Equal 67 $rulings.Count "$Stem fetched ruling sets"
    if ($manifest.baseline_commit -notmatch '^[0-9a-f]{40}$') {
        throw "$Stem has invalid baseline SHA."
    }
    foreach ($file in $manifest.files_sha256.PSObject.Properties) {
        if ($file.Name -notmatch ('^' + [regex]::Escape($Stem) + '[a-z-]*\.(csv|json)$')) {
            throw "$Stem contains an unsafe checksum path '$($file.Name)'."
        }
        $actual = (Get-FileHash (Join-Path $docs $file.Name) -Algorithm SHA256).Hash.ToLowerInvariant()
        Assert-Equal $file.Value $actual "$Stem checksum $($file.Name)"
    }
    Assert-Equal 4 @($manifest.files_sha256.PSObject.Properties).Count "$Stem checksum file count"
    Assert-Equal $population.Count @($population.oracle_id | Sort-Object -Unique).Count "$Stem population Oracle IDs"
    Assert-Equal $population.Count @($population.selection_hash | Sort-Object -Unique).Count "$Stem population hashes"

    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        foreach ($card in $population) {
            $bytes = [Text.Encoding]::UTF8.GetBytes($manifest.selection_salt + $card.oracle_id)
            $expected = [BitConverter]::ToString($sha.ComputeHash($bytes)).Replace('-', '').ToLowerInvariant()
            Assert-Equal $expected $card.selection_hash "$Stem selection hash $($card.oracle_id)"
        }
    }
    finally { $sha.Dispose() }

    $expectedSampleId = 0
    $eligibleCount = 0
    foreach ($stratum in $manifest.strata) {
        $eligible = @($population | Where-Object stratum -eq $stratum.stratum | Sort-Object selection_hash)
        $selected = @($rows | Where-Object stratum -eq $stratum.stratum | Sort-Object { [int]$_.stratum_rank })
        Assert-Equal $stratum.eligible_cards $eligible.Count "$Stem eligible $($stratum.stratum)"
        Assert-Equal $stratum.selected_cards $selected.Count "$Stem selected $($stratum.stratum)"
        Assert-Equal $manifest.rows_per_stratum $selected.Count "$Stem fixed stratum quota"
        if ($stratum.source_cards -lt $eligible.Count) { throw "$Stem invalid population size." }
        $eligibleCount += $eligible.Count
        for ($index = 0; $index -lt $selected.Count; $index++) {
            $expectedSampleId++
            Assert-Equal ($index + 1) ([int]$selected[$index].stratum_rank) "$Stem rank"
            Assert-Equal $expectedSampleId ([int]$selected[$index].sample_id) "$Stem sample order"
            foreach ($field in 'oracle_id', 'selection_hash', 'name') {
                Assert-Equal $eligible[$index].$field $selected[$index].$field "$Stem selected $field"
            }
        }
    }
    Assert-Equal $population.Count $eligibleCount "$Stem accounted population"

    $gapMap = @{}
    foreach ($gap in $gaps) {
        if ($gapMap.ContainsKey($gap.gap)) { throw "$Stem duplicates gap '$($gap.gap)'." }
        if ($gap.disposition -notin @('new_engine', 'existing_dependency', 'deferred')) {
            throw "$Stem unknown disposition '$($gap.disposition)'."
        }
        foreach ($field in 'gap', 'capability', 'title', 'scope_reason', 'code_evidence') {
            if (-not $gap.$field) { throw "$Stem gap '$($gap.gap)' lacks $field." }
        }
        if ($gap.disposition -eq 'new_engine' -and [int]$gap.issue -notin $manifest.new_issue_numbers) {
            throw "$Stem unexpected new issue '$($gap.issue)'."
        }
        if ($gap.disposition -eq 'existing_dependency' -and [int]$gap.issue -notin $manifest.existing_dependencies) {
            throw "$Stem unexpected existing issue '$($gap.issue)'."
        }
        if ($gap.disposition -eq 'deferred' -and $gap.issue) { throw "$Stem deferred gap claims an issue." }
        $gapMap[$gap.gap] = $gap
    }

    $observed = [Collections.Generic.HashSet[string]]::new()
    $stats = @{}
    $anyNew = 0
    $newOnly = 0
    foreach ($row in $rows) {
        foreach ($field in 'rationale', 'code_evidence', 'type_line', 'layout', 'scryfall_id', 'scryfall_uri', 'rulings_uri', 'faces_json') {
            if (-not $row.$field) { throw "$Stem row $($row.sample_id) lacks $field." }
        }
        foreach ($path in $row.code_evidence.Split(';')) {
            if ($path -notmatch '^tricerules/[^:]+\.rs$' -or $path.Contains('..')) {
                throw "$Stem row $($row.sample_id) has invalid code path '$path'."
            }
        }
        $faceData = $row.faces_json | ConvertFrom-Json
        $faces = @($faceData)
        if ($row.layout -in @('adventure', 'transform', 'modal_dfc', 'split', 'flip') -and $faces.Count -lt 2) {
            throw "$Stem row $($row.sample_id) omits multi-face Oracle evidence."
        }
        $rowGaps = @(@($row.primary_gap) + @($row.secondary_gaps -split ';') | Where-Object { $_ })
        Assert-Equal $rowGaps.Count @($rowGaps | Sort-Object -Unique).Count "$Stem duplicate row gaps"
        if ($row.classification -eq 'full_now' -and $rowGaps.Count) { throw "$Stem full row has gaps." }
        if ($row.classification -ne 'full_now' -and -not $row.primary_gap) { throw "$Stem non-full row lacks primary gap." }
        foreach ($gap in $rowGaps) {
            if (-not $gapMap.ContainsKey($gap)) { throw "$Stem unmapped gap '$gap'." }
            [void]$observed.Add($gap)
        }
        $rowIssues = @($rowGaps | ForEach-Object { $gapMap[$_] } | Where-Object disposition -eq 'new_engine' | ForEach-Object { [int]$_.issue } | Sort-Object -Unique)
        foreach ($issue in $rowIssues) {
            if (-not $stats.ContainsKey($issue)) { $stats[$issue] = @{ Occurrences = 0; Sole = 0 } }
            $stats[$issue].Occurrences++
        }
        $otherGaps = @($rowGaps | Where-Object { $gapMap[$_].disposition -ne 'new_engine' })
        if ($rowIssues.Count) { $anyNew++ }
        if ($rowIssues.Count -gt 0 -and $otherGaps.Count -eq 0) { $newOnly++ }
        if ($rowIssues.Count -eq 1 -and $otherGaps.Count -eq 0) { $stats[$rowIssues[0]].Sole++ }
    }
    Assert-Equal $gaps.Count $observed.Count "$Stem exact raw gap coverage"
    Assert-Equal 83 $observed.Count "$Stem raw gap count"

    $reported = [Collections.Generic.HashSet[int]]::new()
    $capabilityRows = Get-MarkdownTableRows -Lines $markdown -Marker 'The normalized capability gaps are:'
    foreach ($row in $capabilityRows) {
        if ($row[3] -notmatch '/issues/(\d+)') { throw "$Stem capability lacks issue URL." }
        $issue = [int]$Matches[1]
        if (-not $reported.Add($issue) -or -not $stats.ContainsKey($issue)) { throw "$Stem duplicate/unknown issue $issue." }
        Assert-Equal $stats[$issue].Occurrences ([int]$row[1]) "$Stem issue $issue occurrences"
        Assert-Equal $stats[$issue].Sole ([int]$row[2]) "$Stem issue $issue sole-gap candidates"
    }
    Assert-Equal $stats.Count $reported.Count "$Stem complete capability statistics"
    $soleTotal = ($stats.Values | ForEach-Object { $_.Sole } | Measure-Object -Sum).Sum
    $markdownText = $markdown -join "`n"
    foreach ($summary in @("**$anyNew cards**", "**$newOnly cards**", "**$soleTotal cards**")) {
        if (-not $markdownText.Contains($summary)) { throw "$Stem omits derived summary '$summary'." }
    }
    foreach ($set in $rulings) {
        if ($set.sample_id -eq 'ferocidon') { continue }
        $sample = @($rows | Where-Object sample_id -eq $set.sample_id)
        Assert-Equal 1 $sample.Count "$Stem ruling sample identity"
        foreach ($ruling in $set.data) {
            Assert-Equal $sample[0].oracle_id $ruling.oracle_id "$Stem ruling Oracle identity"
        }
    }
}

function Test-CurrentStandardCalibration {
    param([string] $Repo, [string] $Stem)

    $docs = Join-Path $Repo 'docs'
    $rows = @(Import-Csv (Join-Path $docs "$Stem.csv"))
    $population = @(Import-Csv (Join-Path $docs "$Stem-population.csv"))
    $gaps = @(Import-Csv (Join-Path $docs "$Stem-gaps.csv"))
    $manifest = Get-Content (Join-Path $docs "$Stem-manifest.json") -Raw | ConvertFrom-Json
    $authority = Get-Content (Join-Path $docs "$Stem-authority.json") -Raw | ConvertFrom-Json

    Assert-Equal 6 $manifest.version "$Stem manifest version"
    Assert-Equal 160 $manifest.sample_size "$Stem manifest sample size"
    Assert-Equal 20 $manifest.rows_per_stratum "$Stem rows per stratum"
    Assert-Equal 8 @($manifest.strata).Count "$Stem manifest strata"
    Assert-Equal 14 @($manifest.new_issue_numbers).Count "$Stem new issue count"
    Assert-Equal 14 @($manifest.new_issues).Count "$Stem new issue metadata count"
    Assert-Equal 'published_and_verified' $manifest.publication_status "$Stem publication status"
    Assert-Equal 610 $population.Count "$Stem population count"
    if ($manifest.baseline_commit -notmatch '^[0-9a-f]{40}$') { throw "$Stem has invalid baseline SHA." }
    if ($manifest.cards_xml_sha256 -notmatch '^[0-9a-f]{64}$') { throw "$Stem has invalid cards.xml SHA." }

    foreach ($file in $manifest.files_sha256.PSObject.Properties) {
        if ($file.Name -notmatch ('^' + [regex]::Escape($Stem) + '[a-z-]*\.(csv|json)$')) {
            throw "$Stem contains an unsafe checksum path '$($file.Name)'."
        }
        $actual = (Get-FileHash (Join-Path $docs $file.Name) -Algorithm SHA256).Hash.ToLowerInvariant()
        Assert-Equal $file.Value $actual "$Stem checksum $($file.Name)"
    }
    Assert-Equal 4 @($manifest.files_sha256.PSObject.Properties).Count "$Stem checksum file count"

    foreach ($field in 'standard_format', 'comprehensive_rules') {
        if ($authority.$field -notmatch '^https://') { throw "$Stem authority lacks $field URL." }
    }
    Assert-Equal 4 @($authority.release_notes.PSObject.Properties).Count "$Stem release-note source count"
    Assert-Equal $manifest.cards_xml_sha256 $authority.local_card_database.sha256 "$Stem cards.xml authority hash"

    Assert-Equal $population.Count @($population.printing_id | Sort-Object -Unique).Count "$Stem population printing IDs"
    Assert-Equal $population.Count @($population.selection_hash | Sort-Object -Unique).Count "$Stem population hashes"
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        foreach ($card in $population) {
            $bytes = [Text.Encoding]::UTF8.GetBytes($manifest.selection_salt + $card.printing_id)
            $expected = [BitConverter]::ToString($sha.ComputeHash($bytes)).Replace('-', '').ToLowerInvariant()
            Assert-Equal $expected $card.selection_hash "$Stem selection hash $($card.printing_id)"
        }
    }
    finally { $sha.Dispose() }

    $expectedSampleId = 0
    $accounted = 0
    foreach ($stratum in $manifest.strata) {
        $eligible = @($population | Where-Object stratum -eq $stratum.stratum | Sort-Object selection_hash)
        $selected = @($rows | Where-Object stratum -eq $stratum.stratum | Sort-Object { [int]$_.stratum_rank })
        Assert-Equal $stratum.eligible_cards $eligible.Count "$Stem eligible $($stratum.stratum)"
        Assert-Equal $stratum.selected_cards $selected.Count "$Stem selected $($stratum.stratum)"
        Assert-Equal 20 $selected.Count "$Stem fixed stratum quota"
        if ($stratum.source_cards -lt $eligible.Count) { throw "$Stem invalid source population size." }
        $accounted += $eligible.Count
        for ($index = 0; $index -lt $selected.Count; $index++) {
            $expectedSampleId++
            Assert-Equal ($index + 1) ([int]$selected[$index].stratum_rank) "$Stem rank"
            Assert-Equal $expectedSampleId ([int]$selected[$index].sample_id) "$Stem sample order"
            foreach ($field in 'printing_id', 'selection_hash', 'name', 'oracle_text') {
                Assert-Equal $eligible[$index].$field $selected[$index].$field "$Stem selected $field"
            }
            Assert-Equal $selected[$index].printing_id $selected[$index].oracle_id "$Stem Oracle alias"
        }
    }
    Assert-Equal $population.Count $accounted "$Stem accounted population"

    $gapMap = @{}
    foreach ($gap in $gaps) {
        if ($gapMap.ContainsKey($gap.gap)) { throw "$Stem duplicates gap '$($gap.gap)'." }
        if ($gap.disposition -notin @('new_engine', 'deferred')) {
            throw "$Stem unknown disposition '$($gap.disposition)'."
        }
        foreach ($field in 'gap', 'capability', 'title', 'scope_reason', 'code_evidence') {
            if (-not $gap.$field) { throw "$Stem gap '$($gap.gap)' lacks $field." }
        }
        if ($gap.disposition -eq 'new_engine' -and [int]$gap.issue -notin $manifest.new_issue_numbers) {
            throw "$Stem unexpected new issue '$($gap.issue)'."
        }
        if ($gap.disposition -eq 'deferred' -and $gap.issue) {
            throw "$Stem deferred gap '$($gap.gap)' unexpectedly has issue '$($gap.issue)'."
        }
        $gapMap[$gap.gap] = $gap
    }
    $observed = [Collections.Generic.HashSet[string]]::new()
    foreach ($row in $rows) {
        foreach ($field in 'rationale', 'code_evidence', 'type_line', 'layout', 'authority_url') {
            if (-not $row.$field) { throw "$Stem row $($row.sample_id) lacks $field." }
        }
        $rowGaps = @(@($row.primary_gap) + @($row.secondary_gaps -split ';') | Where-Object { $_ })
        Assert-Equal $rowGaps.Count @($rowGaps | Sort-Object -Unique).Count "$Stem duplicate row gaps"
        if ($row.classification -eq 'full_now' -and $rowGaps.Count) { throw "$Stem full row has gaps." }
        if ($row.classification -ne 'full_now' -and -not $row.primary_gap) { throw "$Stem non-full row lacks primary gap." }
        foreach ($gap in $rowGaps) {
            if (-not $gapMap.ContainsKey($gap)) { throw "$Stem unmapped gap '$gap'." }
            [void]$observed.Add($gap)
        }
    }
    Assert-Equal $gaps.Count $observed.Count "$Stem exact raw gap coverage"
    Assert-Equal 54 $observed.Count "$Stem raw gap count"
    Assert-Equal 14 @($gaps | Where-Object disposition -eq new_engine).Count "$Stem filed raw gaps"
}

$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$calibrations = @(
    @{
        Csv = Join-Path $repo 'docs\card-coverage-calibration-2026-08.csv'
        Markdown = Join-Path $repo 'docs\card-coverage-calibration-2026-08.md'
    },
    @{
        Csv = Join-Path $repo 'docs\card-coverage-calibration-2026-08-18.csv'
        Markdown = Join-Path $repo 'docs\card-coverage-calibration-2026-08-18.md'
        RequireGapTraceability = $true
    },
    @{
        Csv = Join-Path $repo 'docs\card-coverage-calibration-2026-08-26.csv'
        Markdown = Join-Path $repo 'docs\card-coverage-calibration-2026-08-26.md'
        RequireGapTraceability = $true
        RequireCapabilityStats = $true
    },
    @{
        Csv = Join-Path $repo 'docs\card-coverage-calibration-2026-08-27.csv'
        Markdown = Join-Path $repo 'docs\card-coverage-calibration-2026-08-27.md'
        ExpectedRows = 360
        ExpectedStrata = 12
        RowsPerStratum = 30
    },
    @{
        Csv = Join-Path $repo 'docs\card-coverage-calibration-2026-09-01.csv'
        Markdown = Join-Path $repo 'docs\card-coverage-calibration-2026-09-01.md'
        ExpectedRows = 160
        ExpectedStrata = 8
        RowsPerStratum = 20
    }
)

foreach ($calibration in $calibrations) {
    $parameters = @{ CsvPath = $calibration.Csv; MarkdownPath = $calibration.Markdown }
    foreach ($key in 'RequireGapTraceability', 'RequireCapabilityStats', 'ExpectedRows', 'ExpectedStrata', 'RowsPerStratum') {
        if ($calibration.ContainsKey($key)) { $parameters[$key] = $calibration[$key] }
    }
    Test-Calibration @parameters
}

Test-ExpandedCalibration -Repo $repo -Stem 'card-coverage-calibration-2026-08-27'
Test-CurrentStandardCalibration -Repo $repo -Stem 'card-coverage-calibration-2026-09-01'

Write-Output 'PASS card-coverage calibration consistency'
