//! PointBlitz native viewer: a window that follows the replay server (P1.4, decision 0026).
//!
//! ```text
//! pointblitz-native --server http://127.0.0.1:8700 [--scenario replay|cold|orbit] [--speed 60]
//!                   [--viewpoints bench/viewpoints/flight-01.json] [--view overview_sw]
//!                   [--size 1920x1080] [--no-vsync] [--marks out.jsonl] [--exit-on-end]
//! ```
//!
//! Mouse: drag to orbit, wheel to zoom. Keys 1–8: fixed viewpoints. Esc: quit.
//! Frames are drawn when something changes; `--continuous` draws every frame. `--memory-hints
//! performance` switches wgpu's allocator back to its default (A/B; P1.5 chose `memory`).
//!
//! Marks (`--marks`, one JSON object per line, `t` in ms since start) use the baseline's names
//! where they mean the same thing (decision 0020): `snapshot_received`, `first_chunk`,
//! `delivered`, `submitted`, `presented` (GPU finished the first frame that shows the whole
//! snapshot — synchronised once per snapshot), plus `frames` (all frame start times).
//!
//! `orbit` (decision 0020): the last snapshot as in `cold`, then 120 synchronised frames at each
//! fixed viewpoint into an offscreen target of the window's size and format (like the baseline's
//! un-presented canvas frames): `sync_frame` lines with `cpu` (encode + submit) and `ms` (until the
//! GPU is done).

mod net;

use net::Msg;
use pointblitz_core::renderer::DEPTH_FORMAT;
use pointblitz_core::{Camera, Inserted, Renderer, Scene};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

struct Args {
    server: String,
    scenario: String,
    speed: f64,
    viewpoints: String,
    view: String,
    size: [u32; 2],
    vsync: bool,
    marks: Option<String>,
    exit_on_end: bool,
    /// wgpu allocator preference: memory (default, P1.5) or speed (wgpu's default, A/B).
    memory_hints: wgpu::MemoryHints,
    /// Draw every frame (A/B) instead of only when something changed.
    continuous: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        server: "http://127.0.0.1:8700".into(),
        scenario: "replay".into(),
        speed: 60.0,
        viewpoints: "bench/viewpoints/flight-01.json".into(),
        view: "overview_sw".into(),
        size: [1920, 1080],
        vsync: true,
        marks: None,
        exit_on_end: false,
        memory_hints: wgpu::MemoryHints::MemoryUsage,
        continuous: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut val = || it.next().ok_or(format!("{k} needs a value"));
        match k.as_str() {
            "--server" => a.server = val()?,
            "--scenario" => a.scenario = val()?,
            "--speed" => a.speed = val()?.parse().map_err(|_| "--speed")?,
            "--viewpoints" => a.viewpoints = val()?,
            "--view" => a.view = val()?,
            "--size" => {
                let v = val()?;
                let (w, h) = v.split_once('x').ok_or("--size WxH")?;
                a.size = [
                    w.parse().map_err(|_| "--size")?,
                    h.parse().map_err(|_| "--size")?,
                ];
            }
            "--no-vsync" => a.vsync = false,
            "--marks" => a.marks = Some(val()?),
            "--exit-on-end" => a.exit_on_end = true,
            "--continuous" => a.continuous = true,
            "--memory-hints" => {
                a.memory_hints = match val()?.as_str() {
                    "performance" => wgpu::MemoryHints::Performance,
                    "memory" => wgpu::MemoryHints::MemoryUsage,
                    _ => return Err("--memory-hints performance|memory".into()),
                }
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if !matches!(a.scenario.as_str(), "replay" | "cold" | "orbit") {
        return Err("--scenario replay|cold|orbit".into());
    }
    Ok(a)
}

/// Fixed viewpoints (decision 0017) as cameras.
fn load_views(path: &str) -> Result<Vec<(String, Camera)>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    let fov = json["camera"]["fov_y_deg"].as_f64().ok_or("fov_y_deg")?;
    let v3 = |v: &serde_json::Value| -> Option<[f64; 3]> {
        let a = v.as_array()?;
        Some([
            a.first()?.as_f64()?,
            a.get(1)?.as_f64()?,
            a.get(2)?.as_f64()?,
        ])
    };
    json["viewpoints"]
        .as_array()
        .ok_or("viewpoints")?
        .iter()
        .map(|v| {
            let name = v["name"].as_str().ok_or("name")?.to_string();
            let cam = Camera::new(
                v3(&v["eye"]).ok_or("eye")?,
                v3(&v["target"]).ok_or("target")?,
                v3(&v["up"]).ok_or("up")?,
                fov,
            );
            Ok((name, cam))
        })
        .collect()
}

struct Marks {
    t0: Instant,
    lines: String,
    frames: Vec<f64>,
}

impl Marks {
    fn ms(&self, at: Instant) -> f64 {
        at.duration_since(self.t0).as_secs_f64() * 1e3
    }

    fn add(&mut self, name: &str, at: Instant, extra: &str) {
        let t = self.ms(at);
        writeln!(self.lines, "{{\"name\":\"{name}\",\"t\":{t:.3}{extra}}}").unwrap();
    }
}

struct Gpu {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth: wgpu::TextureView,
    renderer: Renderer,
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

impl Gpu {
    fn new(
        window: Arc<Window>,
        vsync: bool,
        memory_hints: wgpu::MemoryHints,
    ) -> Result<Self, String> {
        let instance = wgpu::Instance::new(
            // WGPU_BACKEND=vulkan|dx12|gl selects the backend (A/B, P1.5).
            wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
        );
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| e.to_string())?;
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let info = adapter.get_info();
        eprintln!(
            "adapter: {} ({:?}, driver {})",
            info.name, info.backend, info.driver_info
        );
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            memory_hints,
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("surface not supported by the adapter")?;
        // PLY colours are sRGB bytes: a non-sRGB format passes them through unchanged, like the
        // capture target (decision 0024) and three.js.
        let caps = surface.get_capabilities(&adapter);
        if let Some(f) = caps.formats.iter().find(|f| !f.is_srgb()) {
            config.format = *f;
        }
        config.present_mode = if vsync {
            wgpu::PresentMode::AutoVsync
        } else {
            wgpu::PresentMode::AutoNoVsync
        };
        surface.configure(&device, &config);
        eprintln!(
            "surface: {}x{} {:?} {:?}",
            config.width, config.height, config.format, config.present_mode
        );
        let depth = depth_view(&device, config.width, config.height);
        let renderer = Renderer::new(&device, config.format);
        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            depth,
            renderer,
        })
    }

    fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
        self.depth = depth_view(&self.device, w, h);
    }
}

/// Minimal executor (same as the core's): no async runtime dependency.
fn block_on<F: std::future::Future>(f: F) -> F::Output {
    use std::task::{Context, Poll, Wake, Waker};
    struct ThreadWaker(std::thread::Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut f = std::pin::pin!(f);
    loop {
        match f.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park(),
        }
    }
}

/// Per-snapshot timing for the end-of-run summary.
#[derive(Default)]
struct SnapTimes {
    kind: String,
    received: Option<Instant>,
    presented: Option<Instant>,
    delivery: String,
    bytes: u64,
}

struct App {
    args: Args,
    proxy: Option<EventLoopProxy<Msg>>,
    gpu: Option<Gpu>,
    scene: Scene,
    views: Vec<(String, Camera)>,
    camera: Camera,
    drag: Option<(f64, f64)>,
    cursor: (f64, f64),
    marks: Marks,
    /// Snapshots whose last chunk is in the scene but not yet presented.
    to_present: Vec<u32>,
    snaps: BTreeMap<u32, SnapTimes>,
    first_chunk_seen: Vec<u32>,
    /// Time spent in Scene::insert (validation + GPU buffer creation) per snapshot.
    upload_ms: BTreeMap<u32, f64>,
    ended: bool,
    errors: usize,
    /// Frames drawn after everything was presented (orbit waits for a few before syncing).
    settled_frames: u32,
}

impl App {
    fn on_msg(&mut self, m: Msg) {
        let now = Instant::now();
        match m {
            Msg::Received { snap, at } => {
                self.marks.add(
                    "snapshot_received",
                    at,
                    &format!(",\"seq\":{},\"kind\":\"{}\"", snap.seq, snap.kind),
                );
                let s = self.snaps.entry(snap.seq).or_default();
                s.kind = snap.kind;
                s.received = Some(at);
            }
            Msg::Chunk {
                seq,
                bytes,
                last,
                at,
            } => {
                if !self.first_chunk_seen.contains(&seq) {
                    self.first_chunk_seen.push(seq);
                    self.marks
                        .add("first_chunk", now, &format!(",\"seq\":{seq}"));
                }
                let Some(gpu) = &self.gpu else { return };
                let t_insert = Instant::now();
                let inserted = self.scene.insert(&gpu.device, &bytes);
                *self.upload_ms.entry(seq).or_default() += t_insert.elapsed().as_secs_f64() * 1e3;
                match inserted {
                    Ok(Inserted::Swapped { from, to }) => {
                        let from = from.map_or("null".into(), |g| g.to_string());
                        self.marks.add(
                            "scene_swap",
                            Instant::now(),
                            &format!(",\"seq\":{seq},\"from\":{from},\"to\":{to}"),
                        );
                    }
                    Ok(Inserted::Stale) => eprintln!("snapshot {seq}: stale chunk dropped"),
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("snapshot {seq}: bad chunk: {e}");
                        self.errors += 1;
                    }
                }
                if last {
                    self.marks
                        .add("last_chunk_received", at, &format!(",\"seq\":{seq}"));
                    self.marks.add(
                        "uploaded",
                        Instant::now(),
                        &format!(
                            ",\"seq\":{seq},\"points\":{},\"gpu_bytes\":{},\"upload_ms\":{:.3}",
                            self.scene.points(),
                            self.scene.gpu_bytes(),
                            self.upload_ms.get(&seq).copied().unwrap_or(0.0)
                        ),
                    );
                    self.to_present.push(seq);
                }
            }
            Msg::Delivered {
                seq,
                delivery,
                generation,
                bytes,
                fetch_start,
                headers_at,
            } => {
                self.marks
                    .add("fetch_start", fetch_start, &format!(",\"seq\":{seq}"));
                self.marks
                    .add("headers", headers_at, &format!(",\"seq\":{seq}"));
                let fetch_ms = now.duration_since(fetch_start).as_secs_f64() * 1e3;
                self.marks.add(
                    "delivered",
                    now,
                    &format!(
                        ",\"seq\":{seq},\"delivery\":\"{delivery}\",\"generation\":{generation},\"bytes\":{bytes},\"fetch_ms\":{fetch_ms:.3}"
                    ),
                );
                let s = self.snaps.entry(seq).or_default();
                s.delivery = delivery;
                s.bytes = bytes;
            }
            Msg::End => {
                self.marks.add("end", now, "");
                self.ended = true;
            }
            Msg::Error(e) => {
                eprintln!("error: {e}");
                self.errors += 1;
            }
        }
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }

    fn redraw(&mut self) {
        let Some(gpu) = &mut self.gpu else { return };
        let frame_start = Instant::now();
        self.marks.frames.push(self.marks.ms(frame_start));
        let tex = match gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                let s = gpu.window.inner_size();
                gpu.resize(s.width, s.height);
                return;
            }
            _ => return,
        };
        let view = tex
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let size = [gpu.config.width, gpu.config.height];
        gpu.renderer.render(
            &gpu.device,
            &gpu.queue,
            &mut enc,
            &view,
            &gpu.depth,
            size,
            &self.camera,
            &self.scene,
        );
        let index = gpu.queue.submit([enc.finish()]);
        if !self.to_present.is_empty() {
            // First frame containing these snapshots: synchronise once (decision 0020).
            let submitted = Instant::now();
            let _ = gpu.device.poll(wgpu::PollType::Wait {
                submission_index: Some(index),
                timeout: None,
            });
            let presented = Instant::now();
            for seq in self.to_present.drain(..) {
                self.marks
                    .add("submitted", submitted, &format!(",\"seq\":{seq}"));
                self.marks
                    .add("presented", presented, &format!(",\"seq\":{seq}"));
                self.snaps.entry(seq).or_default().presented = Some(presented);
            }
        }
        gpu.window.pre_present_notify();
        gpu.queue.present(tex);
    }

    fn summary(&self) -> String {
        let mut s = String::from(
            "| seq | kind | delivery | bytes | received → presented |\n|---:|---|---|---:|---:|\n",
        );
        for (seq, t) in &self.snaps {
            let lat = match (t.received, t.presented) {
                (Some(r), Some(p)) => format!("{:.1} ms", p.duration_since(r).as_secs_f64() * 1e3),
                _ => "—".into(),
            };
            writeln!(
                s,
                "| {seq} | {} | {} | {} | {lat} |",
                t.kind, t.delivery, t.bytes
            )
            .unwrap();
        }
        writeln!(
            s,
            "points on screen: {}, GPU vertex bytes: {}",
            self.scene.points(),
            self.scene.gpu_bytes()
        )
        .unwrap();
        s
    }

    fn finish(&mut self, event_loop: &ActiveEventLoop) {
        eprint!("{}", self.summary());
        if let Some(path) = &self.args.marks {
            let mut out = self.marks.lines.clone();
            let frames: Vec<String> = self
                .marks
                .frames
                .iter()
                .map(|f| format!("{f:.3}"))
                .collect();
            writeln!(out, "{{\"name\":\"frames\",\"t\":[{}]}}", frames.join(",")).unwrap();
            if let Err(e) = std::fs::write(path, out) {
                eprintln!("{path}: {e}");
            }
        }
        event_loop.exit();
    }

    /// Synchronised frames at every fixed viewpoint (SPEC §6.2 frame_time, decision 0020).
    fn sync_orbit(&mut self) {
        const FRAMES_PER_VIEW: usize = 120;
        let Some(gpu) = &mut self.gpu else { return };
        let size = [gpu.config.width, gpu.config.height];
        let color = gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("sync target"),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: gpu.config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.marks.add("sync_start", Instant::now(), "");
        for (name, cam) in &self.views {
            for _ in 0..FRAMES_PER_VIEW {
                let t0 = Instant::now();
                let mut enc = gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
                gpu.renderer.render(
                    &gpu.device,
                    &gpu.queue,
                    &mut enc,
                    &color,
                    &gpu.depth,
                    size,
                    cam,
                    &self.scene,
                );
                let index = gpu.queue.submit([enc.finish()]);
                let t1 = Instant::now();
                let _ = gpu.device.poll(wgpu::PollType::Wait {
                    submission_index: Some(index),
                    timeout: None,
                });
                let t2 = Instant::now();
                let ms = |a: Instant, b: Instant| b.duration_since(a).as_secs_f64() * 1e3;
                writeln!(
                    self.marks.lines,
                    "{{\"name\":\"sync_frame\",\"view\":\"{name}\",\"cpu\":{:.4},\"ms\":{:.4}}}",
                    ms(t0, t1),
                    ms(t0, t2)
                )
                .unwrap();
            }
        }
        self.marks.add("sync_end", Instant::now(), "");
    }

    fn orbit(&mut self, dx: f64, dy: f64) {
        let c = &mut self.camera;
        let up = c.up.normalize();
        let off = c.eye - c.target;
        let yaw = glam_rotate(off, up, -dx * 0.005);
        let right = yaw.cross(up).normalize_or_zero();
        let pitched = glam_rotate(yaw, right, -dy * 0.005);
        // Stop short of the pole so the view never flips.
        if pitched.normalize().dot(up).abs() < 0.995 {
            c.eye = c.target + pitched;
        } else {
            c.eye = c.target + yaw;
        }
    }
}

/// Rotates `v` about unit axis `k` by `angle` (Rodrigues).
fn glam_rotate(v: glam::DVec3, k: glam::DVec3, angle: f64) -> glam::DVec3 {
    let (s, c) = angle.sin_cos();
    v * c + k.cross(v) * s + k * k.dot(v) * (1.0 - c)
}

impl ApplicationHandler<Msg> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("PointBlitz")
            .with_inner_size(PhysicalSize::new(self.args.size[0], self.args.size[1]));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("window: {e}");
                event_loop.exit();
                return;
            }
        };
        match Gpu::new(window, self.args.vsync, self.args.memory_hints.clone()) {
            Ok(g) => self.gpu = Some(g),
            Err(e) => {
                eprintln!("gpu: {e}");
                event_loop.exit();
                return;
            }
        }
        // Refresh rate paces vsync and therefore presentation (record it with the results).
        if let Some(hz) = self
            .gpu
            .as_ref()
            .and_then(|g| g.window.current_monitor())
            .and_then(|m| m.refresh_rate_millihertz())
        {
            eprintln!("display: {:.1} Hz", f64::from(hz) / 1000.0);
            self.marks.add(
                "display",
                Instant::now(),
                &format!(",\"refresh_hz\":{:.1}", f64::from(hz) / 1000.0),
            );
        }
        // Start the network only once the device exists, so no chunk waits for it.
        let proxy = self.proxy.take().expect("proxy set in main");
        let send = move |m: Msg| {
            let _ = proxy.send_event(m);
        };
        let host = match net::host_port(&self.args.server) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("{e}");
                event_loop.exit();
                return;
            }
        };
        self.marks.add(
            "start",
            Instant::now(),
            &format!(",\"scenario\":\"{}\"", self.args.scenario),
        );
        if self.args.scenario != "replay" {
            net::spawn_cold(host, send);
        } else {
            net::spawn_replay(host, self.args.speed, send);
        }
    }

    fn user_event(&mut self, _: &ActiveEventLoop, m: Msg) {
        self.on_msg(m);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let changes_view = matches!(
            event,
            WindowEvent::Resized(_)
                | WindowEvent::CursorMoved { .. }
                | WindowEvent::MouseWheel { .. }
                | WindowEvent::KeyboardInput { .. }
        );
        if changes_view && let Some(g) = &self.gpu {
            g.window.request_redraw();
        }
        match event {
            WindowEvent::CloseRequested => self.finish(event_loop),
            WindowEvent::Resized(s) => {
                if let Some(g) = &mut self.gpu {
                    g.resize(s.width, s.height);
                }
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
                let settled = self.ended && self.to_present.is_empty();
                if settled && self.args.scenario == "orbit" {
                    // Let the swapped snapshot reach the screen first (PR #6 review), then sync.
                    self.settled_frames += 1;
                    if let Some(g) = &self.gpu {
                        g.window.request_redraw();
                    }
                    if self.settled_frames >= 3 {
                        self.sync_orbit();
                        self.finish(event_loop);
                    }
                } else if settled && self.args.exit_on_end {
                    self.finish(event_loop);
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.drag = (state == ElementState::Pressed).then_some(self.cursor);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x, position.y);
                if let Some((x, y)) = self.drag {
                    self.orbit(position.x - x, position.y - y);
                    self.drag = Some(self.cursor);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let steps = match delta {
                    MouseScrollDelta::LineDelta(_, y) => f64::from(y),
                    MouseScrollDelta::PixelDelta(p) => p.y / 100.0,
                };
                let c = &mut self.camera;
                c.eye = c.target + (c.eye - c.target) * 0.9f64.powf(steps);
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                match event.logical_key {
                    Key::Named(NamedKey::Escape) => self.finish(event_loop),
                    Key::Character(ref ch) => {
                        if let Some(i) = ch.parse::<usize>().ok().filter(|i| (1..=9).contains(i))
                            && let Some((name, cam)) = self.views.get(i - 1)
                        {
                            self.camera = cam.clone();
                            self.marks.add(
                                "view",
                                Instant::now(),
                                &format!(",\"view\":\"{name}\""),
                            );
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        // Frames are drawn when something changed (new chunks, camera, size — P1.5). A continuous
        // loop blocks this thread in the swapchain acquire for up to a vsync interval, and chunks
        // that arrive meanwhile wait in the queue. --continuous keeps the old loop for A/B.
        if self.args.continuous
            && let Some(g) = &self.gpu
        {
            g.window.request_redraw();
        }
    }
}

fn main() -> std::process::ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return std::process::ExitCode::from(2);
        }
    };
    let views = match load_views(&args.viewpoints) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return std::process::ExitCode::from(2);
        }
    };
    let Some(camera) = views
        .iter()
        .find(|(n, _)| *n == args.view)
        .map(|(_, c)| c.clone())
    else {
        eprintln!("unknown viewpoint {}", args.view);
        return std::process::ExitCode::from(2);
    };
    let event_loop = match EventLoop::<Msg>::with_user_event().build() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("event loop: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let mut app = App {
        proxy: Some(event_loop.create_proxy()),
        args,
        gpu: None,
        scene: Scene::new(),
        views,
        camera,
        drag: None,
        cursor: (0.0, 0.0),
        marks: Marks {
            t0: Instant::now(),
            lines: String::new(),
            frames: Vec::new(),
        },
        to_present: Vec::new(),
        snaps: BTreeMap::new(),
        first_chunk_seen: Vec::new(),
        upload_ms: BTreeMap::new(),
        ended: false,
        errors: 0,
        settled_frames: 0,
    };
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("event loop: {e}");
        return std::process::ExitCode::FAILURE;
    }
    if app.errors > 0 {
        std::process::ExitCode::FAILURE
    } else {
        std::process::ExitCode::SUCCESS
    }
}
