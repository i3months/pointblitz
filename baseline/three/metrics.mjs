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

  // Frame intervals (requestAnimationFrame deltas): main-thread responsiveness, not GPU cost.
  const deltas = [];
  for (let i = 1; i < frames.length; i++) deltas.push(frames[i] - frames[i - 1]);
  deltas.sort((a, b) => a - b);
  for (const q of [0.5, 0.95, 0.99]) {
    out.push(rec(`frame_time_p${Math.round(q * 100)}`, percentile(deltas, q), 'ms', deltas.length));
  }

  // Synchronised render time per frame at each fixed viewpoint (orbit scenario, decision 0020).
  const sync = raw.syncFrames ?? [];
  if (sync.length) {
    const all = sync.map((f) => f.ms).sort((a, b) => a - b);
    for (const q of [0.5, 0.95, 0.99]) out.push(rec(`render_time_p${Math.round(q * 100)}`, percentile(all, q), "ms", all.length));
    for (const view of [...new Set(sync.map((f) => f.view))]) {
      const v = sync.filter((f) => f.view === view).map((f) => f.ms).sort((a, b) => a - b);
      out.push(rec("render_time_p50_view", percentile(v, 0.5), "ms", v.length, { view }));
    }
  }

  // Main-thread blocks over 50 ms.
  out.push(rec('main_thread_block_count', longtasks.length, 'count'));
  out.push(rec('main_thread_block_ms', longtasks.reduce((a, l) => a + l.ms, 0), 'ms', longtasks.length));

  // Memory: JS heap peak (CDP samples). GPU memory is not observable from a page; record the
  // attribute bytes three.js uploads (position f32×3 + color f32×3) as an estimate.
  if (heap.length) out.push(rec('mem_js_heap_max', Math.max(...heap), 'B', heap.length));
  const lastPoints = by('parse_end').at(-1)?.points;
  if (lastPoints) out.push(rec('mem_gpu_estimate', lastPoints * 24, 'B', 1, { method: 'points × 24 B (position+color float32)' }));

  out.push(rec('stage_fetch_total', stage.fetch.reduce((a, b) => a + b, 0), 'ms', stage.fetch.length));
  out.push(rec('stage_parse_total', stage.parse.reduce((a, b) => a + b, 0), 'ms', stage.parse.length));
  return out;
}
