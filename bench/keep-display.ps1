# Keeps the display on while a measurement runs (decision 0040, rule C3). With the display off after
# the Windows idle timeout, the browser frame clock falls back to a slower software timer (~56.6 Hz
# instead of 60 Hz), which invalidates every rAF-based figure. At start this sends one zero-distance
# mouse move (counts as input, wakes a display that is already off; the pointer does not move), then
# asks Windows to keep the display on for as long as this process lives (the request a video player
# makes). No power setting is changed. usage: powershell -File bench/keep-display.ps1 -ParentPid <pid>
param([int]$ParentPid)
Add-Type -Namespace PB -Name Power -MemberDefinition @'
[DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint flags);
[DllImport("user32.dll")] public static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
'@
[PB.Power]::mouse_event(0x0001, 0, 0, 0, [UIntPtr]::Zero)
$ES_CONTINUOUS = [uint32]'0x80000000'; $ES_DISPLAY_REQUIRED = [uint32]'0x00000002'
[void][PB.Power]::SetThreadExecutionState($ES_CONTINUOUS -bor $ES_DISPLAY_REQUIRED)
while (Get-Process -Id $ParentPid -ErrorAction SilentlyContinue) { Start-Sleep -Seconds 1 }
[void][PB.Power]::SetThreadExecutionState($ES_CONTINUOUS)
