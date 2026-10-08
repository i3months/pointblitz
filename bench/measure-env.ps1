# Measurement environment for one run (decisions 0040, 0041). Started by bench/gpu-watch.mjs; lives
# as long as the parent process. Everything here is per process and ends with the run — no power or
# system setting is changed.
#   display  one zero-distance mouse move (counts as input: wakes a display the idle timeout turned
#            off; the pointer does not move), then SetThreadExecutionState(DISPLAY_REQUIRED) — rule C3.
#   cpu      Windows power throttling (EcoQoS) moves busy threads of background processes to the
#            efficiency cores after ~3 s (~1/3 slower on this PC). Opts out (SetProcessInformation,
#            ProcessPowerThrottling, execution speed) every process of the run: the parent's
#            descendants and this repository's own binaries (target\release). Nothing else is touched.
#   C4       a fixed single-thread calibration loop before the run (iterations/s, best of 3 × 200 ms),
#            and % Processor Performance once a second during it — written to -Out as JSON lines.
# usage: powershell -File bench/measure-env.ps1 -ParentPid <pid> -Out <file.cpu.log> -Repo <dir>
param([int]$ParentPid, [string]$Out, [string]$Repo)
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices; using System.Diagnostics;
public static class Env {
  [StructLayout(LayoutKind.Sequential)] struct PPT { public uint Version; public uint ControlMask; public uint StateMask; }
  [DllImport("kernel32.dll")] static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
  [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
  [DllImport("kernel32.dll")] static extern bool SetProcessInformation(IntPtr h, int cls, ref PPT info, int size);
  [DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint flags);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
  static bool Unthrottle(IntPtr h) { var p = new PPT { Version = 1, ControlMask = 1, StateMask = 0 }; return SetProcessInformation(h, 4, ref p, Marshal.SizeOf(p)); }
  public static bool UnthrottleSelf() { return Unthrottle(GetCurrentProcess()); }
  public static bool UnthrottlePid(int pid) {
    IntPtr h = OpenProcess(0x0200, false, pid); // PROCESS_SET_INFORMATION
    if (h == IntPtr.Zero) return false;
    try { return Unthrottle(h); } finally { CloseHandle(h); }
  }
  // Same loop as the reference in bench/cpu-reference.json — do not change one without the other.
  public static double Calibrate() {
    double best = 0; uint x = 1;
    for (int r = 0; r < 3; r++) {
      var sw = Stopwatch.StartNew(); long n = 0;
      while (sw.ElapsedMilliseconds < 200) { for (int i = 0; i < 100000; i++) x = x * 1103515245u + 12345u; n++; }
      best = Math.Max(best, n / sw.Elapsed.TotalSeconds);
    }
    return x == 0 ? -best : best;
  }
}
'@
$log = { param($o) Add-Content -Path $Out -Value ($o | ConvertTo-Json -Compress) -Encoding utf8 }
[Env]::mouse_event(0x0001, 0, 0, 0, [UIntPtr]::Zero)
[void][Env]::SetThreadExecutionState([uint32]'0x80000002')
[void][Env]::UnthrottleSelf()
& $log @{ type = 'cpu_calibration'; iters_per_s = [math]::Round([Env]::Calibrate(), 1) }
'ready'
$perf = New-Object System.Diagnostics.PerformanceCounter('Processor Information', '% Processor Performance', '_Total')
[void]$perf.NextValue()
$done = @{}
$release = (Join-Path $Repo 'target\release').ToLower()
while (Get-Process -Id $ParentPid -ErrorAction SilentlyContinue) {
  $procs = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, ExecutablePath
  $tree = @{ $ParentPid = $true }
  do {
    $grew = $false
    foreach ($p in $procs) { if ($tree.ContainsKey([int]$p.ParentProcessId) -and -not $tree.ContainsKey([int]$p.ProcessId)) { $tree[[int]$p.ProcessId] = $true; $grew = $true } }
  } while ($grew)
  foreach ($p in $procs) {
    $id = [int]$p.ProcessId
    $ours = $p.ExecutablePath -and $p.ExecutablePath.ToLower().StartsWith($release)
    if (($tree.ContainsKey($id) -or $ours) -and -not $done.ContainsKey($id)) { $done[$id] = [Env]::UnthrottlePid($id) }
  }
  & $log @{ type = 'cpu_perf'; t = [DateTimeOffset]::Now.ToUnixTimeMilliseconds(); pct = [math]::Round($perf.NextValue(), 1); unthrottled = @($done.Values | Where-Object { $_ }).Count; failed = @($done.Values | Where-Object { -not $_ }).Count }
  Start-Sleep -Milliseconds 1000
}
[void][Env]::SetThreadExecutionState([uint32]'0x80000000')
