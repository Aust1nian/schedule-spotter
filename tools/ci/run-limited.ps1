# Runs a command inside a Windows job object with a per-process memory limit
# (spec: public-repo Windows runners have 16 GB, so benchmarks run under a
# 4 GB job-object limit to stand in for an 8 GB laptop). Writes the
# command's stdout to -Out and exits with its exit code.
# Usage: run-limited.ps1 -Exe path -Arguments "args..." -Out file [-LimitMB 4096]
param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [string]$Arguments = "",
  [Parameter(Mandatory = $true)][string]$Out,
  [long]$LimitMB = 4096
)
$ErrorActionPreference = "Stop"

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class SpotterJob {
  [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] static extern IntPtr CreateJobObject(IntPtr a, string name);
  [DllImport("kernel32.dll")] static extern bool SetInformationJobObject(IntPtr job, int infoType, IntPtr info, uint len);
  [DllImport("kernel32.dll")] public static extern bool AssignProcessToJobObject(IntPtr job, IntPtr proc);
  [StructLayout(LayoutKind.Sequential)] struct Basic {
    public long PerProcessUserTimeLimit; public long PerJobUserTimeLimit; public uint LimitFlags;
    public UIntPtr MinimumWorkingSetSize; public UIntPtr MaximumWorkingSetSize; public uint ActiveProcessLimit;
    public UIntPtr Affinity; public uint PriorityClass; public uint SchedulingClass;
  }
  [StructLayout(LayoutKind.Sequential)] struct Io { public ulong a, b, c, d, e, f; }
  [StructLayout(LayoutKind.Sequential)] struct Extended {
    public Basic Basic; public Io Io; public UIntPtr ProcessMemoryLimit; public UIntPtr JobMemoryLimit;
    public UIntPtr PeakProcessMemoryUsed; public UIntPtr PeakJobMemoryUsed;
  }
  public static IntPtr Create(ulong bytes) {
    IntPtr job = CreateJobObject(IntPtr.Zero, null);
    var ext = new Extended();
    ext.Basic.LimitFlags = 0x100; // JOB_OBJECT_LIMIT_PROCESS_MEMORY
    ext.ProcessMemoryLimit = new UIntPtr(bytes);
    int len = Marshal.SizeOf(typeof(Extended));
    IntPtr p = Marshal.AllocHGlobal(len);
    Marshal.StructureToPtr(ext, p, false);
    if (!SetInformationJobObject(job, 9, p, (uint)len)) { throw new Exception("SetInformationJobObject failed"); }
    return job;
  }
}
"@

$job = [SpotterJob]::Create([uint64]($LimitMB * 1MB))
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $Exe
$psi.Arguments = $Arguments
$psi.RedirectStandardOutput = $true
$psi.UseShellExecute = $false
$proc = [System.Diagnostics.Process]::Start($psi)
[void][SpotterJob]::AssignProcessToJobObject($job, $proc.Handle)
$stdout = $proc.StandardOutput.ReadToEnd()
$proc.WaitForExit()
Set-Content -Path $Out -Value $stdout -Encoding utf8
Write-Host "exit $($proc.ExitCode) (memory limit $LimitMB MB)"
exit $proc.ExitCode
