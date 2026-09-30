# Shared exact Cargo test selection. Dot-source; does not execute commands or write files.
function Get-FocusedTestChecks {
    param([Parameter(Mandatory)][string] $Manifest)
    $parsed = Get-Content -LiteralPath $Manifest -Raw | ConvertFrom-Json
    $entries = @($parsed)
    if ($entries.Count -eq 0) { throw 'FocusedTests must contain at least one exact test.' }
    $seen = @{}
    foreach ($entry in $entries) {
        foreach ($field in @('package', 'target', 'test')) {
            if ([string]$entry.$field -cnotmatch '^[A-Za-z0-9_-]+(?:::[A-Za-z0-9_]+)*$') { throw "Invalid focused test $field" }
        }
        if (@($entry.PSObject.Properties.Name | Where-Object { $_ -notin @('package', 'target', 'test', 'features') }).Count) { throw 'Unknown focused test field.' }
        $testArgs = @('test', '--quiet', '-p', [string]$entry.package)
        if ($entry.target -eq 'lib') { $testArgs += '--lib' }
        else { $testArgs += @('--test', [string]$entry.target) }
        if ($entry.features) {
            if ([string]$entry.features -cnotmatch '^[A-Za-z0-9_/-]+(?:,[A-Za-z0-9_/-]+)*$') { throw 'Invalid focused test features.' }
            $testArgs += @('--features', [string]$entry.features)
        }
        $key = "$($entry.package)|$($entry.target)|$($entry.test)|$($entry.features)"
        if ($seen.ContainsKey($key)) { throw "Duplicate focused test: $key" }
        $seen[$key] = $true
        $testArgs += @([string]$entry.test, '--', '--exact')
        [pscustomobject]@{ Label = "Exact test: $($entry.test)"; Args = $testArgs; Exact = $true;
            Package = [string]$entry.package; Features = [string]$entry.features }
    }
}

function Assert-ExactTestResult {
    param([string] $Label, [string] $LogPath)
    if (-not (Select-String -LiteralPath $LogPath -Pattern 'test result: ok\. 1 passed; 0 failed; 0 ignored;')) {
        throw "Expected one executed, non-ignored test for $Label; see $LogPath"
    }
}
