param(
    [Parameter(Mandatory = $true)]
    [string]$QtBin,

    [Parameter(Mandatory = $true)]
    [string]$VcpkgBin,

    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$TestCommand
)

if (-not $TestCommand -or $TestCommand.Count -eq 0) {
    Write-Error "No test executable was supplied."
    exit 2
}

$env:PATH = "$QtBin;$VcpkgBin;$env:PATH"
$env:QT_QPA_PLATFORM = "offscreen"

$testExecutable = $TestCommand[0]
$testArguments = @()
if ($TestCommand.Count -gt 1) {
    $testArguments = $TestCommand[1..($TestCommand.Count - 1)]
}

& $testExecutable @testArguments
exit $LASTEXITCODE
