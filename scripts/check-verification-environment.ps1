# Check capabilities needed by replay's path-safety checks before expensive builds.
[CmdletBinding()]
param(
    [string] $Repository,
    [ValidateRange(0, 1000000)]
    [double] $MinimumFreeGiB = 20,
    [switch] $SkipRustCache
)
$ErrorActionPreference = 'Stop'

function Get-VerificationDrive {
    param([string] $Path)
    # On Unix these paths can live on distinct mounts below '/'. Probe the nearest
    # existing ancestor rather than reducing every path to the filesystem root.
    while (-not [IO.Directory]::Exists($Path)) { $Path = [IO.Path]::GetDirectoryName($Path) }
    if ($isWindowsHost) { $Path = [IO.Path]::GetPathRoot($Path) }
    [IO.DriveInfo]::new($Path)
}

function Reset-RustCacheIfNeeded {
    param([string] $Setting)
    $budgetGiB = 0.0
    if (-not [double]::TryParse($Setting, [Globalization.NumberStyles]::AllowDecimalPoint,
            [Globalization.CultureInfo]::InvariantCulture, [ref] $budgetGiB) -or $budgetGiB -le 0 -or $budgetGiB -gt 1000000) {
        throw 'COCKATRICE_RUST_CACHE_BUDGET_GIB must be a positive size in GiB.'
    }
    $workspace = Join-Path $repo 'tricerules'
    $defaultTarget = Join-Path $workspace 'target'
    Push-Location -LiteralPath $workspace
    try {
        $metadataJson = & cargo metadata --offline --no-deps --format-version 1
        if ($LASTEXITCODE -ne 0) { throw 'Cargo could not identify the local verification cache.' }
        $metadata = ($metadataJson -join "`n") | ConvertFrom-Json
        $pathComparison = if ($isWindowsHost) { [StringComparison]::OrdinalIgnoreCase } else { [StringComparison]::Ordinal }
        if (-not [string]::Equals($metadata.target_directory, $defaultTarget, $pathComparison) -or
                -not [string]::Equals($metadata.build_directory, $defaultTarget, $pathComparison)) {
            throw 'Managed cleanup requires the default Cargo target and build directories.'
        }

        $debug = Join-Path $defaultTarget 'debug'
        # Refuse redirected paths before traversing them. Cargo cleans both its build and target
        # directories, so both effective paths above must identify this checkout's native cache.
        for ($directory = [IO.DirectoryInfo]::new($debug); $directory; $directory = $directory.Parent) {
            $item = Get-Item -LiteralPath $directory.FullName -Force -ErrorAction SilentlyContinue
            if ($item -and (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint))) {
                throw "Managed cleanup refused a non-directory or linked path: $($item.FullName)"
            }
        }
        if (-not (Test-Path -LiteralPath $debug -PathType Container)) { return }
        $tag = Get-Item -LiteralPath (Join-Path $defaultTarget 'CACHEDIR.TAG') -Force
        if ($tag.PSIsContainer -or ($tag.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
                (Get-Content -LiteralPath $tag.FullName -TotalCount 1) -cne 'Signature: 8a477f597d28d172789f06886806bc55') {
            throw "Missing or invalid Cargo cache marker: $($tag.FullName)"
        }
        $pending = [Collections.Generic.Stack[string]]::new()
        $pending.Push($debug)
        $bytes = 0L
        while ($pending.Count) {
            foreach ($entry in Get-ChildItem -LiteralPath $pending.Pop() -Force) {
                if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Linked cache entry: $($entry.FullName)" }
                if ($entry.PSIsContainer) { $pending.Push($entry.FullName) }
                else { $bytes += $entry.Length }
            }
        }
        $freeGiB = (Get-VerificationDrive $debug).AvailableFreeSpace / 1GB
        if ($bytes / 1GB -le $budgetGiB -and ($freeGiB -ge $MinimumFreeGiB -or
                $freeGiB + $bytes / 1GB -lt $MinimumFreeGiB)) { return }
        if (Get-Process cargo,rustc,clippy-driver -ErrorAction SilentlyContinue) {
            throw 'Managed Cargo cleanup requires idle builds. Retry when Cargo and compiler processes have exited.'
        }
        Write-Output ('Recycling the Cargo dev cache ({0:N2} GiB; budget {1:N2} GiB; available {2:N2} GiB).' -f ($bytes / 1GB), $budgetGiB, $freeGiB)
        # Cargo writes its successful cleanup summary to stderr. Windows PowerShell
        # treats that stream as error records when redirected.
        try {
            $ErrorActionPreference = 'Continue'
            & cargo clean --offline --profile dev --target-dir $defaultTarget 2>&1 |
                ForEach-Object { $_.ToString() }
        }
        finally { $ErrorActionPreference = 'Stop' }
        if ($LASTEXITCODE -ne 0 -or (Test-Path -LiteralPath $debug)) { throw 'Cargo dev cache cleanup failed.' }
    }
    finally { Pop-Location }
}

try {
    if (-not $Repository) { $Repository = Split-Path -Parent $PSScriptRoot }
    $repo = [IO.Path]::GetFullPath($Repository)
    $temp = [IO.Path]::GetTempPath()
    $target = $env:CARGO_TARGET_DIR
    if (-not $target) { $target = Join-Path $repo 'tricerules/target' }
    elseif (-not [IO.Path]::IsPathRooted($target)) {
        $target = Join-Path (Join-Path $repo 'tricerules') $target
    }
    $target = [IO.Path]::GetFullPath($target)
    $isWindowsHost = [IO.Path]::DirectorySeparatorChar -eq '\'

    if (-not $SkipRustCache -and $env:COCKATRICE_RUST_CACHE_BUDGET_GIB) {
        Reset-RustCacheIfNeeded $env:COCKATRICE_RUST_CACHE_BUDGET_GIB
    }
    foreach ($path in @($repo, $temp, $target) | Select-Object -Unique) {
        $drive = Get-VerificationDrive $path
        $freeGiB = $drive.AvailableFreeSpace / 1GB
        if ($freeGiB -lt $MinimumFreeGiB) {
            throw ('Insufficient free space on {0}: {1:N1} GiB available, {2:N1} GiB required. Cargo target: {3}. Free space or clean only verified disposable build artifacts before retrying.' -f $drive.Name, $freeGiB, $MinimumFreeGiB, $target)
        }
    }

    if ($isWindowsHost -and -not ('Cockatrice.Verification.PathProbe' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;
namespace Cockatrice.Verification {
    public static class PathProbe {
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern SafeFileHandle CreateFileW(string path, uint access, uint share,
            IntPtr security, uint mode, uint flags, IntPtr template);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern uint GetFinalPathNameByHandleW(SafeFileHandle handle,
            StringBuilder path, uint size, uint flags);
        public static void Canonicalize(string path) {
            using (var handle = CreateFileW(path, 0, 7, IntPtr.Zero, 3, 0x02000000, IntPtr.Zero)) {
                if (handle.IsInvalid) throw new Win32Exception(Marshal.GetLastWin32Error());
                var result = new StringBuilder(32768);
                if (GetFinalPathNameByHandleW(handle, result, (uint)result.Capacity, 0) == 0)
                    throw new Win32Exception(Marshal.GetLastWin32Error());
            }
        }
    }
}
'@
    }
    foreach ($parent in @($repo, $temp) | Select-Object -Unique) {
        $probe = Join-Path $parent ('cockatrice-preflight-' + [guid]::NewGuid())
        try {
            New-Item -ItemType Directory -Path $probe | Out-Null
            try {
                if ($isWindowsHost) { [Cockatrice.Verification.PathProbe]::Canonicalize($probe) }
                else {
                    $canonical = & realpath -e -- $probe
                    if ($LASTEXITCODE -ne 0 -or -not $canonical -or -not [IO.Directory]::Exists($canonical)) {
                        throw 'Native realpath could not resolve the probe directory.'
                    }
                }
            }
            catch {
                throw "Native path canonicalization failed in $parent. Replay requires this for capture path safety. Retry this verification through the supported approval route, or repair the host sandbox permissions. Do not bypass the replay checks. Details: $_"
            }
        }
        finally {
            # This check creates only an empty directory; never recursively clean a parent.
            if (Test-Path -LiteralPath $probe) { Remove-Item -LiteralPath $probe }
        }
    }
    Write-Output "PASS verification environment; Cargo target: $target; minimum free space: $MinimumFreeGiB GiB"
    exit 0
}
catch {
    Write-Output "FAIL verification environment: $_"
    exit 1
}
