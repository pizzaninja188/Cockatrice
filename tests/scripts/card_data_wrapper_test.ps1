param([string] $PowerShell = (Get-Process -Id $PID).Path)
. (Join-Path $PSScriptRoot 'workflow_test_helpers.ps1')

$fixture = New-WorkflowFixture
try {
    # Exercise the real wrapper with a recording native cargo fixture. A nested working
    # directory and paths with spaces distinguish checkout-relative defaults from CWD.
    $result = Invoke-WorkflowFixture $fixture 'gen-cards.ps1' @('--dry-run') (Join-Path $fixture 'nested directory')
    Assert-Workflow ($result.ExitCode -eq 0) "Default input failed: $($result.Output)"
    $trace = @(Read-WorkflowTrace $fixture)
    Assert-Workflow ($trace.Count -eq 1) 'Default input did not invoke cargo exactly once.'
    $arguments = @($trace[0].Arguments)
    $inputIndex = [array]::IndexOf($arguments, '--input')
    Assert-Workflow ($inputIndex -ge 0) 'Default input was not passed to the generator.'
    Assert-Workflow ($arguments[$inputIndex + 1] -eq (Join-Path $fixture 'oracle-cards.jsonl.gz')) 'Wrong default input path.'
    Assert-Workflow ($arguments -contains '--dry-run') 'Generator arguments were lost.'
    $manifestIndex = [array]::IndexOf($arguments, '--manifest-path')
    Assert-Workflow ($manifestIndex -ge 0 -and $arguments[$manifestIndex + 1] -eq (Join-Path $fixture 'tricerules\Cargo.toml')) 'Wrong manifest path.'

    Remove-Item -LiteralPath (Join-Path $fixture 'oracle-cards.jsonl.gz')
    $result = Invoke-WorkflowFixture $fixture 'gen-cards.ps1' @('--dry-run')
    Assert-Workflow ($result.ExitCode -ne 0) 'Missing default input was accepted.'
    Assert-Workflow (@(Read-WorkflowTrace $fixture).Count -eq 1) 'Missing input still invoked cargo.'

    $explicitInput = Join-Path $fixture 'nested directory\custom.jsonl.gz'
    $result = Invoke-WorkflowFixture $fixture 'gen-cards.ps1' @('--input', $explicitInput, '--dry-run')
    Assert-Workflow ($result.ExitCode -eq 0) "Explicit input required the missing default: $($result.Output)"
    $trace = @(Read-WorkflowTrace $fixture)
    Assert-Workflow ($trace.Count -eq 2) 'Explicit input did not invoke cargo exactly once.'
    $arguments = @($trace[1].Arguments)
    Assert-Workflow (@($arguments | Where-Object { $_ -eq '--input' }).Count -eq 1) 'Explicit input was duplicated.'
    $inputIndex = [array]::IndexOf($arguments, '--input')
    Assert-Workflow ($arguments[$inputIndex + 1] -eq $explicitInput) 'Explicit input path was changed.'

    Copy-Item -LiteralPath (Join-Path $sourceRepo 'scripts\fetch-scryfall-bulk.ps1') -Destination (Join-Path $fixture 'scripts')
    $fetchFixture = @{}
    $fetchFixture.requests = [Collections.Generic.List[string]]::new()
    $fetchFixture.downloads = [Collections.Generic.List[string]]::new()
    $fetchFixture.bulkEntries = @(
        [pscustomobject]@{ type = 'oracle_cards'; id = 'cards-id'; updated_at = '2026-09-05T00:00:00Z'; jsonl_download_uri = 'https://fixture.invalid/cards.jsonl.gz'; download_uri = 'https://fixture.invalid/legacy.json' },
        [pscustomobject]@{ type = 'oracle_tags'; id = 'tags-id'; updated_at = '2026-09-04T00:00:00Z'; jsonl_download_uri = 'https://fixture.invalid/tags.jsonl.gz' }
    )
    # The downloader treats response bytes as opaque. Assert exact bytes and independently
    # calculate the provenance hash; no network access or real bulk download is involved.
    $fetchFixture.payload = [byte[]]@(31, 139, 8, 0, 42, 17, 255, 128)
    $fetchFixture.failDownload = $false
    function Invoke-RestMethod {
        param($Uri, $Headers)
        Assert-Workflow ($Uri -eq 'https://api.scryfall.com/bulk-data') "Unexpected index request: $Uri"
        Assert-Workflow ($Headers['User-Agent'] -and $Headers['Accept'] -eq 'application/json') 'Missing request headers.'
        $fetchFixture.requests.Add($Uri)
        return @{ data = $fetchFixture.bulkEntries }
    }
    function Invoke-WebRequest {
        param($Uri, $Headers, $OutFile)
        Assert-Workflow ($Uri -in @('https://fixture.invalid/cards.jsonl.gz', 'https://fixture.invalid/tags.jsonl.gz')) "Unexpected download URI: $Uri"
        Assert-Workflow ($Headers['User-Agent'] -and $Headers['Accept'] -eq 'application/json') 'Missing download headers.'
        $fetchFixture.downloads.Add($Uri)
        if ($fetchFixture.failDownload) { throw 'fixture download failed' }
        [IO.File]::WriteAllBytes($OutFile, $fetchFixture.payload)
    }

    $fetch = Join-Path $fixture 'scripts\fetch-scryfall-bulk.ps1'
    & $fetch
    Assert-Workflow ($fetchFixture.requests.Count -eq 1 -and $fetchFixture.downloads.Count -eq 1) 'Default fetch did not request exactly one index and download.'
    Assert-Workflow ($fetchFixture.downloads[0] -eq $fetchFixture.bulkEntries[0].jsonl_download_uri) 'Fetched the wrong bulk entry.'
    $defaultOutput = Join-Path $fixture 'oracle-cards.jsonl.gz'
    Assert-Workflow ([Convert]::ToBase64String([IO.File]::ReadAllBytes($defaultOutput)) -eq [Convert]::ToBase64String($fetchFixture.payload)) 'Downloaded bytes changed.'
    $metadataBytes = [IO.File]::ReadAllBytes("$defaultOutput.meta.json")
    Assert-Workflow (-not ($metadataBytes[0] -eq 239 -and $metadataBytes[1] -eq 187 -and $metadataBytes[2] -eq 191)) 'Metadata has a UTF-8 BOM.'
    $metadata = [Text.Encoding]::UTF8.GetString($metadataBytes) | ConvertFrom-Json
    foreach ($field in @('type', 'id', 'updated_at', 'jsonl_download_uri')) {
        $value = $metadata.$field
        # PowerShell 7 parses ISO timestamps as DateTime; Windows PowerShell keeps strings.
        if ($value -is [datetime]) { $value = $value.ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ') }
        Assert-Workflow ($value -eq $fetchFixture.bulkEntries[0].$field) "Wrong metadata field: $field"
    }
    $hasher = [Security.Cryptography.SHA256]::Create()
    try { $expectedHash = ([BitConverter]::ToString($hasher.ComputeHash($fetchFixture.payload))).Replace('-', '').ToLowerInvariant() }
    finally { $hasher.Dispose() }
    Assert-Workflow ($metadata.sha256 -ceq $expectedHash) 'Metadata hash does not match downloaded bytes.'
    Assert-Workflow (-not (Test-Path -LiteralPath (Join-Path $fixture 'oracle-tags.jsonl.gz'))) 'Default fetch downloaded tags.'

    $cardsOutput = Join-Path $fixture 'nested directory\cards.gz'
    $tagsOutput = Join-Path $fixture 'nested directory\tags.gz'
    & $fetch -OutFile $cardsOutput -IncludeOracleTags -TagsOutFile $tagsOutput
    Assert-Workflow ($fetchFixture.downloads.Count -eq 3) 'Optional tag fetch did not download both entries.'
    $tagsMetadata = Get-Content -LiteralPath "$tagsOutput.meta.json" -Raw | ConvertFrom-Json
    Assert-Workflow ($tagsMetadata.type -eq 'oracle_tags' -and $tagsMetadata.id -eq 'tags-id' -and $tagsMetadata.sha256 -ceq $expectedHash) 'Tag metadata used the wrong entry or bytes.'
    Assert-Workflow (Test-Path -LiteralPath "$cardsOutput.meta.json") 'Explicit card output did not receive metadata.'

    $fetchFixture.bulkEntries = @([pscustomobject]@{ type = 'oracle_cards'; download_uri = 'https://fixture.invalid/legacy.json' })
    $failedOutput = Join-Path $fixture 'missing-uri.gz'
    $caught = $null
    try { & $fetch -OutFile $failedOutput } catch { $caught = $_ }
    Assert-Workflow ($null -ne $caught -and "$caught" -match 'jsonl_download_uri') 'Missing JSONL URI did not fail clearly.'
    Assert-Workflow ($fetchFixture.downloads.Count -eq 3) 'Missing JSONL URI fell back to a legacy download.'
    Assert-Workflow (-not (Test-Path -LiteralPath "$failedOutput.meta.json")) 'Missing URI published metadata.'

    $fetchFixture.bulkEntries = @([pscustomobject]@{ type = 'oracle_cards'; jsonl_download_uri = 'https://fixture.invalid/cards.jsonl.gz' })
    $fetchFixture.failDownload = $true
    $failedOutput = Join-Path $fixture 'failed-download.gz'
    $caught = $null
    try { & $fetch -OutFile $failedOutput } catch { $caught = $_ }
    Assert-Workflow ($null -ne $caught -and "$caught" -match 'fixture download failed') 'Download failure was swallowed.'
    Assert-Workflow (-not (Test-Path -LiteralPath "$failedOutput.meta.json")) 'Failed download published metadata.'
}
finally { Remove-WorkflowFixture $fixture }
Write-Output 'PASS card data wrapper behavior'
