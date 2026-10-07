// Turns raw measurement hooks into SPEC §6.2 metric records (decision 0020).
// Record: {metric, value, unit, target, device, scenario, commit, samples}

export function percentile(sorted, q) {
  if (sorted.length === 0) return null;
  const i = (sorted.length - 1) * q;
  const lo = Math.floor(i);
  const hi = Math.ceil(i);
  return sorted[lo] + (sorted[hi] - sorted[lo]) * (i - lo);
}

const round = (v, d = 2) => (v == null ? null : Math.round(v * 10 ** d) / 10 ** d);

export function summarize(raw, { device, commit, target = 'three.js' }) {
  const { marks, frames, longtasks = [], heap = [], scenario } = raw;
  const rec = (metric, value, unit, samples = 1, extra = {}) => ({
    metric,
    value: round(value),
    unit,
    target,
    device,
    scenario,
    speed: scenario === 'replay' ? raw.speed : undefined,
    commit,
    samples,
    ...extra,
  });
  const by = (name) => marks.filter((m) => m.name === name);
  const out = [];

  // Bytes: what the client actually downloaded.
  const fetched = by('fetch_end');
  out.push(rec('bytes_total', fetched.reduce((a, m) => a + m.bytes, 0), 'B', fetched.length));

  // Per-event latency: snapshot announced → first frame that shows it.
  const latency = { preview: [], refined: [] };
  const stage = { fetch: [], parse: [] };
  for (const r of by('snapshot_received')) {
    const presented = by('presented').find((m) => m.seq === r.seq);
    const f = by('fetch_end').find((m) => m.seq === r.seq);
    const p = by('parse_end').find((m) => m.seq === r.seq);
    if (!presented) continue;
    latency[r.kind ?? 'refined'].push(presented.t - r.t);
    if (f && p) {
      stage.fetch.push(f.t - by('fetch_start').find((m) => m.seq === r.seq).t);
      stage.parse.push(p.t - f.t);
    }
    out.push(rec('event_latency', presented.t - r.t, 'ms', 1, { seq: r.seq, kind: r.kind, bytes: f?.bytes, points: p?.points }));
  }
  for (const kind of ['preview', 'refined']) {
    const s = latency[kind].sort((a, b) => a - b);
    if (s.length) {
      out.push(rec(`event_latency_${kind}_p50`, percentile(s, 0.5), 'ms', s.length));
      out.push(rec(`event_latency_${kind}_max`, s[s.length - 1], 'ms', s.length));
    }
  }

  // First frame with points, measured from navigation start (performance.now origin).
  const firstPresent = by('presented')[0];
  if (firstPresent) out.push(rec('first_frame', firstPresent.t, 'ms'));

  // Frame intervals (requestAnimationFrame deltas): main-thread responsiveness, not render cost.
  // Named frame_interval_* — SPEC frame_time is the synchronised frame below (PR #6 review H2).
  // Intervals that span the orbit sync loop (rAF loop stopped on purpose) are excluded.
  const syncSpans = by('sync_start').map((s) => [s.t, by('sync_end').find((e) => e.t >= s.t)?.t ?? Infinity]);
  const deltas = [];
  for (let i = 1; i < frames.length; i++) {
    const [a, b] = [frames[i - 1], frames[i]];
    if (syncSpans.some(([s, e]) => a < e && b > s)) continue;
    deltas.push(b - a);
  }
  deltas.sort((a, b) => a - b);
  for (const q of [0.5, 0.95, 0.99]) {
    out.push(rec(`frame_interval_p${Math.round(q * 100)}`, percentile(deltas, q), 'ms', deltas.length));
  }
  if (deltas.length) {
    out.push(rec('frame_interval_max', deltas[deltas.length - 1], 'ms', deltas.length));
  }

  // SPEC §6.2 frame_time (CPU and GPU separately) = synchronised frames at each fixed viewpoint
  // (orbit scenario, decision 0020): frame_time_total = CPU submit + GPU execution + sync;
  // frame_time_cpu = render() call alone; frame_time_gpu_estimate = total − cpu (upper bound).
  const sync = raw.syncFrames ?? [];
  if (sync.length) {
    const series = {
      frame_time_total: sync.map((f) => f.ms),
      frame_time_cpu: sync.map((f) => f.cpu),
      frame_time_gpu_estimate: sync.map((f) => f.ms - f.cpu),
    };
    for (const [name, values] of Object.entries(series)) {
      const s = values.sort((a, b) => a - b);
      for (const q of [0.5, 0.95, 0.99]) out.push(rec(`${name}_p${Math.round(q * 100)}`, percentile(s, q), 'ms', s.length));
    }
    for (const view of [...new Set(sync.map((f) => f.view))]) {
      const v = sync.filter((f) => f.view === view).map((f) => f.ms).sort((a, b) => a - b);
      out.push(rec('frame_time_total_p50_view', percentile(v, 0.5), 'ms', v.length, { view }));
    }
  }

  // Event latency split: submitted (render() returned) vs presented (GPU done) for the swap frame.
  for (const s of by('submitted')) {
    const p = by('presented').find((m) => m.seq === s.seq);
    if (p) out.push(rec('swap_frame_gpu_wait', p.t - s.t, 'ms', 1, { seq: s.seq }));
  }

  // Main-thread blocks over 50 ms. Long tasks inside the orbit sync loop are a measurement
  // artefact (120 synchronised frames per task) and are excluded (PR #6 review).
  const blocks = longtasks.filter((l) => !syncSpans.some(([a, b]) => l.t >= a - 1 && l.t <= b));
  out.push(rec('main_thread_block_count', blocks.length, 'count'));
  out.push(rec('main_thread_block_ms', blocks.reduce((a, l) => a + l.ms, 0), 'ms', blocks.length));

  // Memory: JS heap peak (CDP samples). GPU memory is not observable from a page; record the
  // attribute bytes three.js uploads as an estimate. PLYLoader keeps the file's normals as a
  // third float32 attribute, so it uploads position + normal + color = 36 B/point (PR #5 review).
  // mem_cpu = renderer process private bytes from the OS (includes ArrayBuffer, typed arrays and
  // wasm memory, which JSHeapUsedSize does not count — PR #6 review H1). JS heap stays as a
  // secondary metric. The GPU process figure is reported separately.
  const proc = raw.proc ?? { renderer: [], gpu: [] };
  if (proc.renderer.length) {
    out.push(rec('mem_cpu', Math.max(...proc.renderer.map((s) => s.private)), 'B', proc.renderer.length, { method: 'renderer process private bytes (max of 250 ms samples)' }));
    if (scenario === 'memtest') {
      // Self-check: private bytes just before vs after holding the 200 MB Float32Array.
      const a = by('alloc_start')[0], e = by('alloc_end')[0];
      const at = (m) => raw.timeOrigin + m.t;
      const before = proc.renderer.filter((x) => x.ts < at(a)).map((x) => x.private);
      const after = proc.renderer.filter((x) => x.ts > at(e) + 250).map((x) => x.private);
      if (before.length && after.length) {
        out.push(rec('memtest_cpu_delta', Math.max(...after) - before[before.length - 1], 'B', after.length, { allocated: e.bytes }));
        const hb = heap.length ? heap : [0];
        out.push(rec('memtest_js_heap_max', Math.max(...hb), 'B', hb.length));
      }
    }
    out.push(rec('mem_renderer_working_set_max', Math.max(...proc.renderer.map((s) => s.working)), 'B', proc.renderer.length));
  }
  if (proc.gpu.length) out.push(rec('mem_gpu_process_private_max', Math.max(...proc.gpu.map((s) => s.private)), 'B', proc.gpu.length));
  if (heap.length) out.push(rec('mem_js_heap_max', Math.max(...heap), 'B', heap.length));
  const last = by('parse_end').at(-1);
  if (last?.points) {
    const perPoint = 12 * (last.attributes ?? 3);
    out.push(rec('mem_gpu_estimate', last.points * perPoint, 'B', 1, {
      method: `points × ${perPoint} B (${last.attributeNames ?? 'position,normal,color'} float32×3)`,
    }));
  }

  out.push(rec('stage_fetch_total', stage.fetch.reduce((a, b) => a + b, 0), 'ms', stage.fetch.length));
  out.push(rec('stage_parse_total', stage.parse.reduce((a, b) => a + b, 0), 'ms', stage.parse.length));
  return out;
}
