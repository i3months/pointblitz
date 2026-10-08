// GPU time of a batch of frames on WebGL2 (decision 0049): EXT_disjoint_timer_query_webgl2's
// TIME_ELAPSED around the batch, on the same context the frames are drawn with. Shared by the
// PointBlitz page (wgpu's GL backend issues its GL calls inside submit) and the three.js baselines,
// so every WebGL2 target is timed the same way.

/** Returns a timer for `gl`, or null when the extension is not exposed. */
export function glTimer(gl) {
  const ext = gl?.getExtension('EXT_disjoint_timer_query_webgl2');
  if (!ext) return null;
  const pending = [];
  return {
    /** Starts timing; call before the batch's first draw. */
    begin() {
      const q = gl.createQuery();
      gl.beginQuery(ext.TIME_ELAPSED_EXT, q);
      return q;
    },
    /** Stops timing; `target.gpu` gets ms per frame once the result is in (see finish). */
    end(q, frames, target) {
      gl.endQuery(ext.TIME_ELAPSED_EXT);
      pending.push({ q, frames, target });
    },
    /** Waits for every result (results arrive only after returning to the event loop). A result
     *  read while the GPU reported a disjoint event is dropped: `target.gpu` stays unset. */
    async finish() {
      for (let tries = 0; pending.length && tries < 2000; tries++) {
        await new Promise((r) => setTimeout(r, 1));
        const disjoint = gl.getParameter(ext.GPU_DISJOINT_EXT);
        for (let i = pending.length - 1; i >= 0; i--) {
          const p = pending[i];
          if (!gl.getQueryParameter(p.q, gl.QUERY_RESULT_AVAILABLE)) continue;
          if (!disjoint) p.target.gpu = gl.getQueryParameter(p.q, gl.QUERY_RESULT) / 1e6 / p.frames;
          gl.deleteQuery(p.q);
          pending.splice(i, 1);
        }
      }
      return pending.length === 0;
    },
  };
}
