# Run with no competing writers. Temporarily changes data, restores exact bytes in finally.
$ErrorActionPreference = 'Stop'
$repo = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$manifest = Join-Path $repo 'tricerules/Cargo.toml'
$runner = Join-Path $repo 'scripts/run-quiet-command.ps1'
$results = [Collections.Generic.List[object]]::new()
function Invoke-MeasuredBuild([string] $Phase) {
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $result = & $runner -Label "Incremental card data: $Phase" -WorkingDirectory $repo `
        -Executable cargo -ArgumentList @('build', '--manifest-path', $manifest,
        '-p', 'tricerules-server', '--message-format=json') -AsResultObject
    $timer.Stop()
    Write-Host $result.Summary
    if ($result.ExitCode -ne 0) {
        Get-Content -LiteralPath $result.LogPath
        throw "Cargo build failed in $Phase with exit $($result.ExitCode)"
    }
    $artifacts = @(Get-Content -LiteralPath $result.LogPath | ForEach-Object {
        if ($_ -match '^\s*\{') { $_ | ConvertFrom-Json }
    } | Where-Object { $_.reason -eq 'compiler-artifact' -and $_.target.kind -contains 'lib' })
    $record = [ordered]@{ Phase = $Phase; Seconds = $timer.Elapsed.TotalSeconds; Log = $result.LogPath }
    foreach ($name in @('tricerules_card_model', 'tricerules_core', 'tricerules_cards', 'tricerules_server')) {
        $artifact = @($artifacts | Where-Object { $_.target.name -eq $name })
        if ($artifact.Count -ne 1) { throw "Expected one library artifact for $name" }
        $record[$name] = [bool] $artifact[0].fresh
    }
    $results.Add([pscustomobject] $record)
    return [pscustomobject] $record
}
$report = Join-Path $repo ('build/verification-logs/incremental-card-data-' + [guid]::NewGuid() + '.json')
try {
    $null = Invoke-MeasuredBuild 'baseline'
    $card = Get-ChildItem -LiteralPath (Join-Path $repo 'tricerules/tricerules-cards/data') -Filter '*.ron' -Recurse |
        Sort-Object FullName | Select-Object -First 1
    foreach ($path in @($card.FullName, (Join-Path $repo 'tricerules/tricerules-cards/presentation/oracle_fingerprints.tsv'))) {
        $bytes = [IO.File]::ReadAllBytes($path)
        try {
            [IO.File]::WriteAllBytes($path, [byte[]] ($bytes + [byte]10))
            $changed = Invoke-MeasuredBuild ([IO.Path]::GetFileName($path))
            if (-not $changed.tricerules_core -or -not $changed.tricerules_card_model) {
                throw 'A data-only edit recompiled stable model or engine code.'
            }
            if ($changed.tricerules_cards -or $changed.tricerules_server) {
                throw 'The changed payload did not invalidate its final consumers.'
            }
        }
        finally { [IO.File]::WriteAllBytes($path, $bytes) }
        $null = Invoke-MeasuredBuild 'restored'
    }
    Write-Host 'PASS: RON and presentation edits reuse the model and engine libraries.'
}
finally {
    $results | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $report -Encoding UTF8
    Write-Host "Measurement report: $report"
}
