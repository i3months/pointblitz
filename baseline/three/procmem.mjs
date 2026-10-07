// Samples OS-level memory of Chrome's renderer and GPU processes (decision 0020, PR #6 review H1).
//
// JSHeapUsedSize (CDP) does not count ArrayBuffer / typed array / wasm memory, so it under-reports
// both three.js (its 67 MB file buffer and 90 MB attributes) and a wasm renderer (almost
// everything). Private bytes of the renderer process include all of them.
//
// One long-lived PowerShell process prints "pid private workingset" lines every interval, so the
// sampling cost does not depend on spawning a process per sample.

import { spawn } from 'node:child_process';

/** Finds renderer and GPU process ids via the browser-level CDP domain SystemInfo. */
export async function chromeProcessIds(browser) {
  const session = await browser.newBrowserCDPSession();
  const { processInfo } = await session.send('SystemInfo.getProcessInfo');
  await session.detach();
  return {
    renderer: processInfo.filter((p) => p.type === 'renderer').map((p) => p.id),
    gpu: processInfo.filter((p) => p.type === 'GPU').map((p) => p.id),
  };
}

/** Starts sampling; returns stop() → {pid: [{private, working}]} */
export function sampleProcesses(pids, intervalMs = 250) {
  const list = pids.join(',');
  const script = `while ($true) { foreach ($p in Get-Process -Id ${list} -ErrorAction SilentlyContinue) { "$($p.Id) $($p.PrivateMemorySize64) $($p.WorkingSet64)" }; [Console]::Out.Flush(); Start-Sleep -Milliseconds ${intervalMs} }`;
  const ps = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', script], { stdio: ['ignore', 'pipe', 'ignore'] });
  const samples = {};
  let buf = '';
  ps.stdout.on('data', (d) => {
    buf += d;
    const lines = buf.split(/\r?\n/);
    buf = lines.pop();
    for (const line of lines) {
      const [pid, priv, ws] = line.trim().split(/\s+/).map(Number);
      if (!pid) continue;
      (samples[pid] ??= []).push({ ts: Date.now(), private: priv, working: ws });
    }
  });
  return () => {
    ps.kill();
    return samples;
  };
}
