//! PointBlitz browser target (wasm): the same `pointblitz-core` on a canvas (P2, decisions 0028, 0029).
//!
//! Browser-only code is compiled for wasm32 alone; the host build keeps the crate checkable.
//! Built with `web/build.sh` (cargo → wasm32 → `wasm-bindgen --target web`), loaded by `web/`.
//!
//! JS drives the loop: it hands chunks to [`Viewer::insert`], sets the camera, and calls
//! [`Viewer::render`] from `requestAnimationFrame` only when something changed (decision 0027).
//! [`Viewer::gpu_done`] resolves when the GPU has finished the frames submitted so far — the
//! `presented` mark (decision 0020).

#[cfg(target_arch = "wasm32")]
mod viewer;
#[cfg(target_arch = "wasm32")]
pub use viewer::Viewer;

// Used by the wasm viewer; on the host only its tests use it.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod oneshot;

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn json_strings_are_escaped() {
        assert_eq!(super::json_str("a\"b\\c\n"), "\"a\\\"b\\\\c\\u000a\"");
    }
}
