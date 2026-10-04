# Keep each rustfmt invocation within Windows' command-line length limit.
function Get-RustFormatChecks {
    param([Parameter(Mandatory)][string] $Repository)
    $workspaceRoot = Join-Path $Repository 'tricerules'
    $manifest = [IO.File]::ReadAllText((Join-Path $workspaceRoot 'Cargo.toml'))
    $workspace = [regex]::Match($manifest, '(?ms)^\[workspace\]\s*\r?\n(?<body>.*?)(?=^\[|\z)')
    $workspaceBody = [regex]::Replace($workspace.Groups['body'].Value, '(?m)#.*$', '')
    $members = [regex]::Match($workspaceBody, '(?ms)^\s*members\s*=\s*\[(?<members>.*?)\]')
    if (-not $workspace.Success -or -not $members.Success) { throw 'Missing explicit Rust workspace members.' }
    $memberText = $members.Groups['members'].Value
    $memberPaths = @([regex]::Matches($memberText, '"(?<path>[^"\r\n]+)"') | ForEach-Object { $_.Groups['path'].Value })
    if (-not $memberPaths.Count -or [regex]::Replace($memberText, '"[^"\r\n]+"|[\s,]', '') -ne '') {
        throw 'Unsupported Rust workspace member syntax; formatting coverage cannot be established.'
    }
    $seen = @{}
    foreach ($member in $memberPaths) {
        $packageManifest = Join-Path (Join-Path $workspaceRoot $member) 'Cargo.toml'
        $packageText = [IO.File]::ReadAllText($packageManifest)
        $package = [regex]::Match($packageText, '(?ms)^\[package\]\s*\r?\n(?<body>.*?)(?=^\[|\z)')
        $name = [regex]::Match($package.Groups['body'].Value, '(?m)^\s*name\s*=\s*"(?<name>[A-Za-z0-9_-]+)"\s*(?:#.*)?$')
        if (-not $package.Success -or -not $name.Success) { throw "Missing explicit Rust package name: $packageManifest" }
        $packageName = $name.Groups['name'].Value
        if ($seen.ContainsKey($packageName)) { throw "Duplicate Rust workspace package: $packageName" }
        $seen[$packageName] = $true
        @{ Label = "Rust formatting ($packageName)";
            Executable = (Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/powershell.exe');
            Args = @('-NoProfile', '-File', (Join-Path $Repository 'scripts/check-rust-format.ps1'), '-Package', $packageName);
            Exact = $false }
    }
}
