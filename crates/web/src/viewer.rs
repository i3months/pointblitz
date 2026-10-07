//! The browser viewer: a WebGPU surface on a canvas around the core `Scene` and `Renderer`.

use crate::json_str;
use crate::oneshot::Signal;
use pointblitz_core::renderer::DEPTH_FORMAT;
use pointblitz_core::{Camera, Inserted, Renderer, Scene};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Viewer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth: wgpu::TextureView,
    renderer: Renderer,
    scene: Scene,
    camera: Camera,
    adapter: String,
}

fn err(what: &str, e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&format!("{what}: {e}"))
}

fn depth_view(device: &wgpu::Device, w: u32, h: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

#[wasm_bindgen]
impl Viewer {
    /// Opens WebGPU on `canvas` (its `width`/`height` attributes set the drawing size).
    ///
    /// `backend`: `auto` (WebGPU when the browser has it, otherwise WebGL2), `webgpu` or `webgl`.
    /// A canvas keeps the first context type it was given, so the choice is made before the surface.
    pub async fn create(
        canvas: web_sys::HtmlCanvasElement,
        backend: String,
    ) -> Result<Viewer, JsValue> {
        let (w, h) = (canvas.width().max(1), canvas.height().max(1));
        let webgpu = match backend.as_str() {
            "webgpu" => true,
            "webgl" => false,
            "auto" | "" => wgpu::util::is_browser_webgpu_supported().await,
            other => return Err(JsValue::from_str(&format!("unknown backend {other}"))),
        };
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = if webgpu {
            wgpu::Backends::BROWSER_WEBGPU
        } else {
            wgpu::Backends::GL
        };
        let instance = wgpu::Instance::new(desc);
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| err("surface", e))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|e| err("adapter", e))?;
        let info = adapter.get_info();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                // WebGL2 cannot meet wgpu's default limits; ask only for what WebGL2 guarantees,
                // raised to the adapter's texture sizes.
                required_limits: if webgpu {
                    wgpu::Limits::default()
                } else {
                    wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits())
                },
                ..Default::default()
            })
            .await
            .map_err(|e| err("device", e))?;
        let mut config = surface
            .get_default_config(&adapter, w, h)
            .ok_or_else(|| JsValue::from_str("surface not supported"))?;
        // PLY colours are sRGB bytes: a non-sRGB format passes them through, like native and the
        // capture target (decision 0024). The browser's preferred format is bgra8unorm.
        let caps = surface.get_capabilities(&adapter);
        if let Some(f) = caps.formats.iter().find(|f| !f.is_srgb()) {
            config.format = *f;
        }
        config.alpha_mode = wgpu::CompositeAlphaMode::Opaque;
        surface.configure(&device, &config);
        let depth = depth_view(&device, w, h);
        let renderer = Renderer::new(&device, config.format);
        let adapter = format!(
            "{{\"backend\":\"{:?}\",\"format\":\"{:?}\",\"width\":{w},\"height\":{h},\"name\":{}}}",
            info.backend,
            config.format,
            json_str(&info.name)
        );
        Ok(Viewer {
            surface,
            device,
            queue,
            config,
            depth,
            renderer,
            scene: Scene::new(),
            camera: Camera::new([0.0, -10.0, 10.0], [0.0; 3], [0.0, 0.0, 1.0], 50.0),
            adapter,
        })
    }

    /// Backend, surface format and size as JSON.
    pub fn info(&self) -> String {
        self.adapter.clone()
    }

    /// Adds one chunk (decision 0022). Returns what it did: `appended`, `pending`, `swapped`, `stale`.
    pub fn insert(&mut self, chunk: &[u8]) -> Result<String, JsValue> {
        let r = self
            .scene
            .insert(&self.device, chunk)
            .map_err(|e| err("chunk", e))?;
        Ok(match r {
            Inserted::Appended => "appended",
            Inserted::Pending => "pending",
            Inserted::Swapped { .. } => "swapped",
            Inserted::Stale => "stale",
        }
        .into())
    }

    pub fn set_camera(&mut self, eye: &[f64], target: &[f64], up: &[f64], fov_y_deg: f64) {
        let v = |a: &[f64]| [a[0], a[1], a[2]];
        self.camera = Camera::new(v(eye), v(target), v(up), fov_y_deg);
    }

    /// Draws one frame to the canvas.
    pub fn render(&mut self) -> Result<(), JsValue> {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            other => return Err(JsValue::from_str(&format!("surface texture: {other:?}"))),
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        self.renderer.render(
            &self.device,
            &self.queue,
            &mut enc,
            &view,
            &self.depth,
            [self.config.width, self.config.height],
            &self.camera,
            &self.scene,
        );
        self.queue.submit([enc.finish()]);
        self.queue.present(frame);
        Ok(())
    }

    /// Resolves when the GPU has finished everything submitted so far.
    pub fn gpu_done(&self) -> js_sys::Promise {
        let signal = Signal::default();
        let fire = signal.clone();
        self.queue.on_submitted_work_done(move || fire.fire());
        wasm_bindgen_futures::future_to_promise(async move {
            signal.await;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Lets wgpu run completion callbacks without blocking. WebGPU calls them by itself; on WebGL2
    /// they only run when the device is polled, so `gpu_done` would never resolve (P2.4). Call it
    /// once per animation frame.
    pub fn poll(&self) {
        let _ = self.device.poll(wgpu::PollType::Poll);
    }

    /// Points on screen.
    pub fn points(&self) -> f64 {
        self.scene.points() as f64
    }

    /// Vertex buffer bytes on the GPU (shown + pending generations).
    pub fn gpu_bytes(&self) -> f64 {
        self.scene.gpu_bytes() as f64
    }
}
