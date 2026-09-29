function Assert-CardEvidenceReference {
    param([string] $Reference, [string[]] $Available, [string[]] $Ignored)
    if ([string]::IsNullOrWhiteSpace($Reference)) { throw 'Empty semantic fixture reference' }
    if ($Available -cnotcontains $Reference) { throw "Semantic fixture test does not exist: $Reference" }
    if ($Ignored -ccontains $Reference) { throw "Semantic fixture test is ignored: $Reference" }
}
function Get-CardEvidenceExecutables {
    param([string[]] $Lines, [string[]] $Targets)
    $executables = @{}
    foreach ($line in $Lines) {
        # Cargo's JSON stream may be interleaved with human-readable diagnostics.
        if (-not $line.StartsWith('{')) { continue }
        $artifact = $line | ConvertFrom-Json
        if ($artifact.reason -ne 'compiler-artifact' -or
            $artifact.target.kind -cnotcontains 'test' -or -not $artifact.profile.test -or
            $Targets -cnotcontains $artifact.target.name) { continue }
        $name = [string] $artifact.target.name
        if ([string]::IsNullOrWhiteSpace($artifact.executable)) { throw "Missing test executable: $name" }
        if ($executables.ContainsKey($name)) { throw "Ambiguous test executable: $name" }
        $executables[$name] = [string] $artifact.executable
    }
    foreach ($target in $Targets) {
        if (-not $executables.ContainsKey($target)) { throw "Cargo did not produce test target: $target" }
    }
    return $executables
}
Export-ModuleMember -Function Assert-CardEvidenceReference, Get-CardEvidenceExecutables
