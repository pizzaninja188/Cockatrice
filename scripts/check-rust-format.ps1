# Cargo fmt puts every package target into one Windows command line. Keep the same
# target/edition coverage while invoking check-only rustfmt in bounded chunks.
[CmdletBinding()]
param([Parameter(Mandatory)][string] $Package)
$ErrorActionPreference = 'Stop'
$workspaceRoot = Join-Path (Split-Path -Parent $PSScriptRoot) 'tricerules'
Push-Location $workspaceRoot
try {
    $cargo = Get-Command cargo -CommandType Application -ErrorAction Stop | Select-Object -First 1
    $formatter = Get-Command rustfmt -CommandType Application -ErrorAction Stop | Select-Object -First 1
    # Successful native stderr is diagnostic output, not a PowerShell exception.
    $ErrorActionPreference = 'Continue'
    $metadataText = & $cargo.Source metadata --format-version 1 --no-deps
    $metadataCode = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($metadataCode -ne 0) { exit $metadataCode }
    $metadata = ($metadataText -join "`n") | ConvertFrom-Json
    $selected = @($metadata.packages | Where-Object { $_.name -eq $Package })
    if ($selected.Count -ne 1) { throw "Expected exactly one Cargo package: $Package" }
    $targets = @($selected[0].targets | Sort-Object src_path, edition -Unique)
    if (-not $targets.Count) { throw "No Rust formatting targets for $Package" }
    foreach ($target in $targets) {
        if ($target.edition -notin @('2015', '2018', '2021', '2024') -or
            -not $target.src_path -or -not (Test-Path -LiteralPath $target.src_path -PathType Leaf)) {
            throw "Invalid Rust formatting target for $Package"
        }
    }
    foreach ($group in @($targets | Group-Object edition)) {
        $paths = @($group.Group.src_path)
        $index = 0
        while ($index -lt $paths.Count) {
            $chunk = @('--check', '--edition', $group.Name)
            $length = 128
            while ($index -lt $paths.Count) {
                # Paths cannot contain double quotes on Windows. Include quoting and spaces.
                $nextLength = $paths[$index].Length + 3
                if ($length + $nextLength -gt 6000) {
                    if ($chunk.Count -eq 3) { throw 'Rust formatting target exceeds argument budget.' }
                    break
                }
                $chunk += $paths[$index]
                $length += $nextLength
                $index++
            }
            $ErrorActionPreference = 'Continue'
            & $formatter.Source @chunk
            $formatCode = $LASTEXITCODE
            $ErrorActionPreference = 'Stop'
            if ($formatCode -ne 0) { exit $formatCode }
        }
    }
    Write-Output "Checked $($targets.Count) Cargo formatting targets for $Package."
    exit 0
}
catch {
    Write-Error $_ -ErrorAction Continue
    exit 1
}
finally { Pop-Location }
