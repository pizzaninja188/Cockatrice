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

function Artifact([string] $Target, [string] $Executable, [string] $Kind = 'test') {
    @{ reason = 'compiler-artifact'; target = @{ name = $Target; kind = @($Kind) };
        profile = @{ test = $true }; executable = $Executable } | ConvertTo-Json -Compress
}
$lines = @('compiler diagnostic', (Artifact 'alpha' 'C:/path with spaces/alpha.exe'),
    (Artifact 'beta' 'C:/beta.exe'), (Artifact 'alpha' 'C:/library.exe' 'lib'))
$executables = Get-CardEvidenceExecutables $lines @('alpha', 'beta')
if ($executables['alpha'] -cne 'C:/path with spaces/alpha.exe' -or $executables.Count -ne 2) {
    throw 'Executable selection mixed targets or lost a spaced path'
}
foreach ($invalid in @(
    @{ Lines = $lines; Targets = @('missing') },
    @{ Lines = @((Artifact 'alpha' '')); Targets = @('alpha') },
    @{ Lines = @((Artifact 'alpha' 'C:/one.exe'), (Artifact 'alpha' 'C:/two.exe')); Targets = @('alpha') }
)) {
    $rejected = $false
    try { $null = Get-CardEvidenceExecutables $invalid.Lines $invalid.Targets }
    catch { $rejected = $true }
    if (-not $rejected) { throw 'Accepted missing or ambiguous test executable' }
}
Write-Output 'PASS current Cargo artifacts resolve exact, unambiguous test targets'
