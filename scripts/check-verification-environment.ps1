# Check capabilities needed by replay's path-safety checks before expensive builds.
[CmdletBinding()]
param(
    [string] $Repository,
    [ValidateRange(0, 1000000)]
    [double] $MinimumFreeGiB = 20
)
$ErrorActionPreference = 'Stop'
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

    foreach ($root in @($repo, $temp, $target) | ForEach-Object { [IO.Path]::GetPathRoot($_) } | Select-Object -Unique) {
        $drive = [IO.DriveInfo]::new($root)
        $freeGiB = $drive.AvailableFreeSpace / 1GB
        if ($freeGiB -lt $MinimumFreeGiB) {
            throw ('Insufficient free space on {0}: {1:N1} GiB available, {2:N1} GiB required. Cargo target: {3}. Free space or clean only verified disposable build artifacts before retrying; no files were deleted.' -f $root, $freeGiB, $MinimumFreeGiB, $target)
        }
    }

    if (-not ('Cockatrice.Verification.PathProbe' -as [type])) {
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
            try { [Cockatrice.Verification.PathProbe]::Canonicalize($probe) }
            catch {
                throw "Windows path canonicalization failed in $parent. Replay requires this for capture path safety. Retry this verification through the supported approval route, or repair the host sandbox permissions. Do not bypass the replay checks. Details: $_"
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
