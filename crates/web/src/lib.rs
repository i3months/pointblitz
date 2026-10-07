//! PointBlitz browser target (wasm): the same `pointblitz-core` on a canvas (P2, decision 0028).
//!
//! Browser-only code is compiled for wasm32 alone; the host build keeps the crate checkable.
//! Built with `web/build.sh` (cargo → wasm32 → `wasm-bindgen --target web`), loaded by `web/index.html`.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// Opens a WebGPU adapter for `canvas` and describes it as JSON
/// (`{"name", "backend", "driver", "driver_info"}`) — the P2.1 check that the browser gives us a
/// real GPU through WebGPU.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn adapter_info(canvas: web_sys::HtmlCanvasElement) -> Result<String, JsValue> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::BROWSER_WEBGPU;
    let instance = wgpu::Instance::new(desc);
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
        .map_err(|e| JsValue::from_str(&format!("surface: {e}")))?;
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        })
        .await
        .map_err(|e| JsValue::from_str(&format!("adapter: {e}")))?;
    let info = adapter.get_info();
    Ok(format!(
        "{{\"name\":{},\"backend\":\"{:?}\",\"driver\":{},\"driver_info\":{}}}",
        json_str(&info.name),
        info.backend,
        json_str(&info.driver),
        json_str(&info.driver_info)
    ))
}

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
