<# Offline preflight, frozen review bundles, and measured commands. No Git writes. #>
# No advanced parameter binding: --out must reach Python unchanged under PS 5.1.
[string[]] $CommandArgs = $args
$ErrorActionPreference = 'Stop'
& python (Join-Path $PSScriptRoot 'authoring-batch.py') @CommandArgs
exit $LASTEXITCODE
