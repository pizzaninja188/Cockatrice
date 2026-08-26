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

        [bool] $RequireCapabilityStats = $false
    )

    $displayName = Split-Path -Leaf $CsvPath
    $rows = @(Import-Csv -LiteralPath $CsvPath)
    Assert-Equal 180 $rows.Count "$displayName row count"

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
    Assert-Equal 3 $strata.Count "$displayName stratum count"
    foreach ($stratum in $strata) {
        Assert-Equal 60 $stratum.Count "$displayName $($stratum.Name) row count"
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
    }
)

foreach ($calibration in $calibrations) {
    $requireGapTraceability = $calibration.ContainsKey('RequireGapTraceability') -and
        $calibration.RequireGapTraceability
    $requireCapabilityStats = $calibration.ContainsKey('RequireCapabilityStats') -and
        $calibration.RequireCapabilityStats
    Test-Calibration `
        -CsvPath $calibration.Csv `
        -MarkdownPath $calibration.Markdown `
        -RequireGapTraceability $requireGapTraceability `
        -RequireCapabilityStats $requireCapabilityStats
}

Write-Output 'PASS card-coverage calibration consistency'
