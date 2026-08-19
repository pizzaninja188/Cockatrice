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
        [string] $MarkdownPath
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

    foreach ($identityColumn in 'sample_id', 'selection_hash', 'oracle_id') {
        $values = @($rows | ForEach-Object { $_.$identityColumn })
        Assert-Equal $rows.Count @($values | Sort-Object -Unique).Count "$displayName unique $identityColumn count"
        if ($values -contains '') {
            throw "$displayName contains an empty $identityColumn."
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
    }
)

foreach ($calibration in $calibrations) {
    Test-Calibration -CsvPath $calibration.Csv -MarkdownPath $calibration.Markdown
}

Write-Output 'PASS card-coverage calibration consistency'
