function Assert-CardEvidenceReference {
    param([string] $Reference, [string[]] $Available, [string[]] $Ignored)
    if ([string]::IsNullOrWhiteSpace($Reference)) { throw 'Empty semantic fixture reference' }
    if ($Available -cnotcontains $Reference) { throw "Semantic fixture test does not exist: $Reference" }
    if ($Ignored -ccontains $Reference) { throw "Semantic fixture test is ignored: $Reference" }
}
Export-ModuleMember -Function Assert-CardEvidenceReference
