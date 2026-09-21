$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot '../../scripts/card-evidence.psm1') -Force
function Expect-Rejection {
    param([string] $Reference, [string[]] $Available, [string[]] $Ignored = @())
    $rejected = $false
    try { Assert-CardEvidenceReference $Reference $Available $Ignored }
    catch { $rejected = $true }
    if (-not $rejected) { throw "Accepted invalid evidence reference: $Reference" }
}
Assert-CardEvidenceReference 'scenario actual::case' @('scenario actual::case') @()
Expect-Rejection 'scenario nonexistent::case' @('scenario actual::case')
Expect-Rejection 'scenario actual::case' @('scenario actual::case') @('scenario actual::case')
Expect-Rejection 'wrong_target::case' @('right_target::case')
Expect-Rejection '' @()
Write-Output 'PASS card evidence rejects missing, wrong-target, and ignored tests'
