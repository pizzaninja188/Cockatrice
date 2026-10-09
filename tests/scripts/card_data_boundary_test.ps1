# Guard the incremental-build boundary between engine code and embedded card data.
$ErrorActionPreference = 'Stop'
$repo = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$manifest = Join-Path $repo 'tricerules/Cargo.toml'
$raw = & cargo metadata --format-version 1 --manifest-path $manifest
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$metadata = ($raw -join "`n") | ConvertFrom-Json
$corePackages = @($metadata.packages | Where-Object name -eq 'tricerules-core')
$cardPackages = @($metadata.packages | Where-Object name -eq 'tricerules-cards')
if ($corePackages.Count -ne 1 -or $cardPackages.Count -ne 1) { throw 'Missing or ambiguous workspace package identity.' }
$core = $corePackages[0]
$cards = $cardPackages[0]
$nodes = @{}
foreach ($node in $metadata.resolve.nodes) { $nodes[$node.id] = $node }
$pending = [Collections.Generic.Queue[string]]::new()
$seen = [Collections.Generic.HashSet[string]]::new()
$pending.Enqueue($core.id)
while ($pending.Count -gt 0) {
    $id = $pending.Dequeue()
    if (-not $seen.Add($id)) { continue }
    if ($id -eq $cards.id) {
        throw 'Core production dependencies reach the embedded card-data crate.'
    }
    foreach ($dependency in $nodes[$id].deps) {
        # Dev dependencies may use the real corpus for engine characterization tests.
        if (@($dependency.dep_kinds | Where-Object { $_.kind -ne 'dev' }).Count -gt 0) {
            $pending.Enqueue($dependency.pkg)
        }
    }
}
Write-Host 'PASS: core production dependency graph excludes embedded card data.'
