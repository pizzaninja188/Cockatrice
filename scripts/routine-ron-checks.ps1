# Read-only input capture and conservative eligibility for the opt-in routine RON gate.
function Get-VerificationInputSnapshot {
    param([Parameter(Mandatory)][string] $Repository)
    $repo = [IO.Path]::GetFullPath($Repository)
    $names = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $gitNames = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $raw = (@(& git -C $repo -c core.quotepath=false ls-files --cached --others --exclude-standard -z) -join "`n")
    if ($LASTEXITCODE -ne 0 -or -not $raw.EndsWith("`0")) { throw 'Could not inventory Git inputs for routine verification.' }
    foreach ($name in ($raw -split "`0")) {
        if (-not $name) { continue }
        if ($name -match '[^\x20-\x7e]' -or [IO.Path]::IsPathRooted($name) -or $name -match '(^|/)\.\.(/|$)') { throw 'Unsupported Git input path.' }
        $normalized = $name.Replace('\', '/')
        if (-not $gitNames.Add($normalized)) { throw "Duplicate Git input path: $normalized" }
        $null = $names.Add($normalized)
    }
    # Gitlinks are tracked directories, so hashing leaf files alone would miss a
    # committed submodule change. Record their index identities and require them clean.
    $gitlinks = @{}
    $index = (@(& git -C $repo ls-files --stage -z) -join "`n")
    if ($LASTEXITCODE -ne 0 -or ($index -and -not $index.EndsWith("`0"))) { throw 'Could not inventory Git index entries.' }
    foreach ($entry in ($index -split "`0")) {
        if ($entry -cmatch '^160000 ([0-9a-f]{40,64}) ([0-3])\t(.+)$') {
            $objectId = $Matches[1]; $stage = $Matches[2]; $path = $Matches[3].Replace('\', '/')
            if ($stage -ne '0' -or -not $gitNames.Contains($path) -or $gitlinks.ContainsKey($path)) { throw "Invalid or unmerged Git submodule: $path" }
            $status = @(& git -C $repo status --porcelain=v1 --ignore-submodules=none --untracked-files=all -- $path)
            if ($LASTEXITCODE -ne 0 -or $status.Count -ne 0) { throw "Dirty or unreadable Git submodule: $path" }
            $sha = [Security.Cryptography.SHA256]::Create()
            try { $gitlinks[$path] = ([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes("gitlink:$objectId")))).Replace('-', '') } finally { $sha.Dispose() }
        }
    }
    # The card build script enumerates RON from disk, including ignored files. Tests may
    # likewise read ignored fixture/source files under these directories.
    $directories = @('tricerules/tricerules-cards/data', 'tricerules/tricerules-cards/authoring/review-maps')
    $workspace = Join-Path $repo 'tricerules'
    if (Test-Path -LiteralPath $workspace -PathType Container) {
        foreach ($package in Get-ChildItem -LiteralPath $workspace -Directory -Force) {
            foreach ($child in @('src', 'tests')) {
                if (Test-Path -LiteralPath (Join-Path $package.FullName $child) -PathType Container) {
                    $directories += "tricerules/$($package.Name)/$child"
                }
            }
        }
    }
    foreach ($directory in $directories) {
        $root = Join-Path $repo $directory
        if (-not (Test-Path -LiteralPath $root -PathType Container)) { continue }
        foreach ($item in Get-ChildItem -LiteralPath $root -Recurse -Force) {
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Cannot fingerprint a reparse point: $($item.FullName)" }
            if (-not $item.PSIsContainer) {
                $relative = $item.FullName.Substring($repo.TrimEnd('\', '/').Length + 1).Replace('\', '/')
                $null = $names.Add($relative)
            }
        }
    }
    $files = [Collections.Generic.List[object]]::new()
    foreach ($name in @($names | Sort-Object)) {
        if ($gitlinks.ContainsKey($name)) {
            $files.Add([pscustomobject][ordered]@{ Path = $name; Sha256 = $gitlinks[$name]; GitListed = $true })
            continue
        }
        $path = Join-Path $repo $name
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { continue }
        $item = Get-Item -LiteralPath $path -Force
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Cannot fingerprint a reparse point: $path" }
        $files.Add([pscustomobject][ordered]@{ Path = $name; Sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash; GitListed = $gitNames.Contains($name) })
    }

    # Hash values, rather than writing possibly sensitive environment values to the log.
    $context = [Collections.Generic.List[string]]::new()
    foreach ($variable in @(Get-ChildItem Env: | Where-Object {
        $_.Name -match '^(RUST|CARGO|GIT_|TRICERULES_|CC|CXX|AR|CFLAGS|CXXFLAGS|LDFLAGS|PKG_CONFIG|LIBCLANG|PROTOC|CLANG|BINDGEN|OPENSSL|VCPKG|DEP_|HOST_|TARGET_)' -or
            $_.Name -in @('PATH', 'LINK', 'INCLUDE', 'LIB', 'LIBPATH')
    } | Sort-Object Name)) { $context.Add("env:$($variable.Name)=$($variable.Value)") }
    foreach ($tool in @('cargo', 'rustc')) {
        $command = Get-Command $tool -CommandType Application -ErrorAction Stop | Select-Object -First 1
        $version = & $command.Source --version --verbose 2>&1
        if ($LASTEXITCODE -ne 0) { throw "Could not identify $tool." }
        $context.Add("tool:$tool=$($command.Source):$($version -join "`n")")
    }
    # Cargo loads ancestor and user configuration even when Git ignores it.
    $configRoots = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $parent = [IO.DirectoryInfo]::new($workspace)
    while ($null -ne $parent) { $null = $configRoots.Add((Join-Path $parent.FullName '.cargo')); $parent = $parent.Parent }
    $cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path ([Environment]::GetFolderPath('UserProfile')) '.cargo' }
    if (-not [IO.Path]::IsPathRooted($cargoHome)) { throw 'Relative CARGO_HOME is ambiguous between verification working directories.' }
    $null = $configRoots.Add([IO.Path]::GetFullPath($cargoHome))
    foreach ($root in @($configRoots | Sort-Object)) {
        foreach ($leaf in @('config', 'config.toml')) {
            $path = Join-Path $root $leaf
            $hash = if (Test-Path -LiteralPath $path -PathType Leaf) { (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash } else { 'MISSING' }
            $context.Add("config:$path=$hash")
        }
    }
    $bytes = [Text.Encoding]::UTF8.GetBytes(($context -join "`n"))
    $sha = [Security.Cryptography.SHA256]::Create()
    try { $contextHash = ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '') } finally { $sha.Dispose() }
    [pscustomobject][ordered]@{ Version = 1; ContextSha256 = $contextHash; Files = @($files.ToArray()) }
}

function ConvertTo-VerificationSnapshotJson {
    param([Parameter(Mandatory)] $Snapshot)
    ConvertTo-Json -InputObject $Snapshot -Depth 8 -Compress
}

function Read-RoutineRonBaseline {
    param([Parameter(Mandatory)][string] $SummaryPath, [Parameter(Mandatory)][object[]] $ExpectedSteps)
    $path = [IO.Path]::GetFullPath($SummaryPath)
    $summary = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
    if ($summary.Status -cne 'Pass' -or $null -eq $summary.ExitCode -or $summary.ExitCode -ne 0 -or
            $summary.Side -cnotin @('Rust', 'Both') -or $summary.CardData -isnot [bool] -or -not $summary.CardData -or
            $summary.RecordRoutineBaseline -isnot [bool] -or -not $summary.RecordRoutineBaseline -or
            $summary.RoutineRon -isnot [bool] -or $summary.RoutineRon -or
            -not $summary.StartedAt -or -not $summary.CompletedAt) { throw 'Baseline is not a recorded successful full Rust/CardData run.' }
    $record = $summary.RoutineBaseline
    if ($null -eq $record -or $record.Version -ne 1 -or $record.SnapshotFile -cne 'routine-inputs.json' -or
            [string]$record.StartSha256 -cnotmatch '^[A-F0-9]{64}$' -or $record.StartSha256 -cne $record.EndSha256) {
        throw 'Baseline does not bind matching start and end inputs.'
    }
    $actualSteps = @($summary.Steps)
    if ($actualSteps.Count -ne $ExpectedSteps.Count) { throw 'Baseline has an incomplete verification plan.' }
    $directory = Split-Path -Parent $path
    $directoryPrefix = [IO.Path]::GetFullPath($directory).TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    for ($i = 0; $i -lt $ExpectedSteps.Count; $i++) {
        $actual = $actualSteps[$i]; $expected = $ExpectedSteps[$i]
        if ($actual.Label -cne $expected.Label -or $actual.Executable -cne $expected.Executable -or
                $actual.WorkingDirectory -cne $expected.WorkingDirectory -or $actual.RequireE2E -isnot [bool] -or
                $actual.RequireE2E -ne $expected.RequireE2E -or $actual.Exact -isnot [bool] -or $actual.Exact -ne $expected.Exact -or
                (ConvertTo-Json -InputObject @($actual.Arguments) -Compress) -cne (ConvertTo-Json -InputObject @($expected.Arguments) -Compress) -or
                $actual.Status -cne 'Pass' -or $null -eq $actual.ExitCode -or $actual.ExitCode -ne 0 -or -not $actual.LogPath) {
            throw "Baseline verification step is incomplete or changed: $($expected.Label)"
        }
        $log = [IO.Path]::GetFullPath([string]$actual.LogPath)
        if (-not $log.StartsWith($directoryPrefix, [StringComparison]::OrdinalIgnoreCase) -or -not (Test-Path -LiteralPath $log -PathType Leaf)) {
            throw "Baseline verification log is missing: $($expected.Label)"
        }
    }
    $snapshotPath = Join-Path $directory $record.SnapshotFile
    if (-not (Test-Path -LiteralPath $snapshotPath -PathType Leaf) -or
            (Get-FileHash -LiteralPath $snapshotPath -Algorithm SHA256).Hash -cne $record.StartSha256) { throw 'Baseline snapshot is missing or changed.' }
    $snapshot = Get-Content -LiteralPath $snapshotPath -Raw | ConvertFrom-Json
    if ($snapshot.Version -ne 1 -or [string]$snapshot.ContextSha256 -cnotmatch '^[A-F0-9]{64}$' -or -not @($snapshot.Files).Count) {
        throw 'Baseline snapshot has an invalid schema.'
    }
    return $snapshot
}

function Get-RoutineRonPlan {
    param([Parameter(Mandatory)][string] $Repository, [Parameter(Mandatory)] $Baseline, [Parameter(Mandatory)] $Current)
    if ($Baseline.Version -ne 1 -or $Current.Version -ne 1 -or -not $Baseline.ContextSha256 -or
            $Baseline.ContextSha256 -cne $Current.ContextSha256) { throw 'Verification environment or snapshot version changed; run the full gate.' }
    $before = @{}
    $after = @{}
    $listed = @{}
    foreach ($pair in @(@($Baseline.Files), @($Current.Files))) {
        $table = if ($before.Count -eq 0) { $before } else { $after }
        foreach ($file in $pair) {
            $path = [string]$file.Path
            if (-not $path -or $table.ContainsKey($path) -or [string]$file.Sha256 -cnotmatch '^[A-F0-9]{64}$' -or $file.GitListed -isnot [bool]) { throw 'Invalid or duplicate snapshot entry.' }
            $table[$path] = "$($file.Sha256)|$($file.GitListed)"
            if ($table -eq $after) { $listed[$path] = $file.GitListed }
        }
    }
    if (-not $before.Count -or -not $after.Count) { throw 'Empty verification snapshot.' }
    $allPaths = @($before.Keys) + @($after.Keys) | Sort-Object -Unique
    $changed = @($allPaths | Where-Object { -not $before.ContainsKey($_) -or -not $after.ContainsKey($_) -or $before[$_] -cne $after[$_] })
    foreach ($path in $changed) {
        if ($after.ContainsKey($path) -and -not $listed[$path]) { throw "Ignored input change requires the full gate: $path" }
    }
    $cards = [Collections.Generic.List[string]]::new()
    foreach ($path in $changed) {
        if ($path -cmatch '^tricerules/tricerules-cards/data/([^/]+)\.ron$') {
            if (-not $after.ContainsKey($path)) { throw "Deleted card definition requires the full gate: $path" }
            $cards.Add($Matches[1])
        }
    }
    if (-not $cards.Count) { throw 'Routine verification requires at least one changed handwritten card RON.' }
    $allowed = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($path in @('tricerules/CARDS.md', 'tricerules/tricerules-cards/presentation/oracle_fingerprints.tsv',
            'tricerules/tricerules-cards/authoring/partial-cards.tsv', 'tricerules/tricerules-cards/authoring/presentation-exceptions.tsv')) { $null = $allowed.Add($path) }
    $tests = [Collections.Generic.List[object]]::new()
    $seenTests = @{}
    foreach ($card in $cards) {
        $ron = "tricerules/tricerules-cards/data/$card.ron"
        $map = "tricerules/tricerules-cards/authoring/review-maps/$card.json"
        $null = $allowed.Add($ron)
        $null = $allowed.Add($map)
        if (-not $after.ContainsKey($map) -or -not $listed[$map]) { throw "Missing or ignored review map for $card." }
        $definition = Get-Content -LiteralPath (Join-Path $Repository $map) -Raw | ConvertFrom-Json
        if ($definition.complete_definition_review_confirmed -isnot [bool] -or -not $definition.complete_definition_review_confirmed) { throw "Unconfirmed review map for $card." }
        if ($definition.PSObject.Properties.Name -notcontains 'tokens' -or @($definition.tokens).Count -ne 0) { throw "Shared token dependencies require the full gate: $card" }
        if (-not @($definition.semantic_fixtures).Count) { throw "Missing semantic fixtures for $card." }
        $scenario = $false
        foreach ($fixture in $definition.semantic_fixtures) {
            $reference = [string]$fixture.test
            if ($reference -cmatch '^scenario ([A-Za-z0-9_]+)::([A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)*)$') {
                $module = $Matches[1]; $package = 'tricerules-core'; $target = 'scenario'; $name = "$module`::$($Matches[2])"
                $path = "tricerules/tricerules-core/tests/scenario/$module.rs"
                $scenario = $true
            }
            elseif ($reference -cmatch '^(batch_registry|deck_registry|generated_registry|mechanic_registry|standard_registry)::([A-Za-z0-9_]+)::([A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)*)$') {
                $target = $Matches[1]; $module = $Matches[2]; $name = "$module`::$($Matches[3])"; $package = 'tricerules-cards'
                $path = "tricerules/tricerules-cards/tests/$module.rs"
            }
            else { throw "Unsupported semantic fixture for $card`: $reference" }
            if ($module -match '(?i)(helper|common|fixture)') { throw "Shared test helper requires the full gate: $path" }
            if (-not $after.ContainsKey($path)) { throw "Missing referenced leaf test module: $path" }
            if (-not $before.ContainsKey($path)) { throw "A new test module requires the full gate: $path" }
            $null = $allowed.Add($path)
            $key = "$package|$target|$name"
            if (-not $seenTests.ContainsKey($key)) {
                $seenTests[$key] = $true
                $tests.Add([pscustomobject]@{ Package = $package; Target = $target; Name = $name })
            }
        }
        if (-not $scenario) { throw "A referenced actual-card scenario is required for $card." }
    }
    foreach ($path in $changed) {
        if (-not $allowed.Contains($path)) { throw "Change requires the full gate: $path" }
    }
    [pscustomobject]@{ Cards = @($cards.ToArray()); Tests = @($tests.ToArray()); ChangedPaths = @($changed) }
}
