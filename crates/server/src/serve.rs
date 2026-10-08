//! `serve` (P3.2, decision 0036): follow the replay server, render and encode at a fixed rate,
//! stream frames over WebSocket, take camera input back.
//!
//! ```text
//! pointblitz-server serve --replay http://127.0.0.1:8700 [--port 8720] [--speed 60] [--qp 18]
//!                         [--fps 60] [--viewpoints bench/viewpoints/flight-01.json] [--view overview_sw]
//!                         [--exit-after-end <s>] [--wait-for-client]
//! ```
//!
//! Each frame is one binary WebSocket message: `u32 LE metadata length`, the metadata (UTF-8 JSON),
//! then the H.264 access unit (Annex B). Metadata: `frame` (counter), `idr`, `t_ms` (server clock,
//! ms since start, when the frame's tick began), `render_ms`, `encode_ms`, `visible` (snapshots that
//! became fully visible in this frame), `snapshot` (newest fully visible snapshot), `input` (last
//! applied input id), `points`.
//!
//! `--wait-for-client` starts following the replay only once a client is connected, so a test or
//! measurement client sees every snapshot from the first.
//!
//! Clients send text messages: `{"id":n,"type":"orbit","dx":px,"dy":px}`, `{"id":n,"type":"zoom",
//! "steps":s}`, `{"id":n,"type":"view","name":"north"}`. A client joining forces an IDR.

use crate::nvenc::{Config, Encoder};
use glam::DVec3;
use pointblitz_core::{Camera, Headless, Inserted, Scene};
use pointblitz_io::client::{self, Msg};
use std::fmt::Write as _;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tungstenite::Message;

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

/// A camera change from a client.
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Orbit { dx: f64, dy: f64 },
    Zoom { steps: f64 },
    View { name: String },
}

/// Parses a client text message into (id, input).
pub fn parse_input(text: &str) -> Option<(u64, Input)> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let id = v["id"].as_u64()?;
    let input = match v["type"].as_str()? {
        "orbit" => Input::Orbit {
            dx: v["dx"].as_f64()?,
            dy: v["dy"].as_f64()?,
        },
        "zoom" => Input::Zoom {
            steps: v["steps"].as_f64()?,
        },
        "view" => Input::View {
            name: v["name"].as_str()?.to_string(),
        },
        _ => return None,
    };
    Some((id, input))
}

/// Rotates `v` about unit axis `k` by `angle` (Rodrigues).
fn rotate(v: DVec3, k: DVec3, angle: f64) -> DVec3 {
    let (s, c) = angle.sin_cos();
    v * c + k.cross(v) * s + k * k.dot(v) * (1.0 - c)
}

/// Same camera controls as the native viewer (drag to orbit, wheel to zoom).
pub fn apply(camera: &mut Camera, input: &Input, views: &[(String, Camera)]) {
    match input {
        Input::Orbit { dx, dy } => {
            let up = camera.up.normalize();
            let yaw = rotate(camera.eye - camera.target, up, -dx * 0.005);
            let right = yaw.cross(up).normalize_or_zero();
            let pitched = rotate(yaw, right, -dy * 0.005);
            camera.eye = camera.target
                + if pitched.normalize().dot(up).abs() < 0.995 {
                    pitched
                } else {
                    yaw
                };
        }
        Input::Zoom { steps } => {
            camera.eye = camera.target + (camera.eye - camera.target) * 0.9f64.powf(*steps);
        }
        Input::View { name } => {
            if let Some((_, c)) = views.iter().find(|(n, _)| n == name) {
                *camera = c.clone();
            }
        }
    }
}

/// Builds one frame message: u32 LE metadata length, metadata, H.264.
pub fn frame_message(meta: &str, h264: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + meta.len() + h264.len());
    out.extend((meta.len() as u32).to_le_bytes());
    out.extend(meta.as_bytes());
    out.extend(h264);
    out
}

type Clients = Arc<Mutex<Vec<mpsc::Sender<Arc<Vec<u8>>>>>>;

/// One thread per client: write frames from the channel, read input with a short timeout.
fn client_thread(
    stream: TcpStream,
    frames: mpsc::Receiver<Arc<Vec<u8>>>,
    inputs: mpsc::Sender<(u64, Input)>,
) {
    let _ = stream.set_nodelay(true);
    let Ok(mut ws) = tungstenite::accept(stream) else {
        return;
    };
    let _ = ws
        .get_mut()
        .set_read_timeout(Some(Duration::from_millis(1)));
    loop {
        match ws.read() {
            Ok(Message::Text(t)) => {
                if let Some(i) = parse_input(t.as_str()) {
                    let _ = inputs.send(i);
                }
            }
            Ok(Message::Close(_)) => return,
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return,
        }
        // Send everything queued; the newest frame matters most, but P frames depend on every
        // earlier one, so none is dropped here.
        while let Ok(f) = frames.try_recv() {
            if ws.send(Message::Binary(f.as_ref().clone().into())).is_err() {
                return;
            }
        }
    }
}

struct Views {
    list: Vec<(String, Camera)>,
}

fn load_views(path: &str) -> Result<(Views, u32, u32), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    let cam = &json["camera"];
    let fov = cam["fov_y_deg"].as_f64().ok_or("fov_y_deg")?;
    let w = cam["width"].as_u64().ok_or("width")? as u32;
    let h = cam["height"].as_u64().ok_or("height")? as u32;
    let v3 = |v: &serde_json::Value| -> Option<[f64; 3]> {
        let a = v.as_array()?;
        Some([
            a.first()?.as_f64()?,
            a.get(1)?.as_f64()?,
            a.get(2)?.as_f64()?,
        ])
    };
    let list = json["viewpoints"]
        .as_array()
        .ok_or("viewpoints")?
        .iter()
        .map(|v| {
            Ok((
                v["name"].as_str().ok_or("name")?.to_string(),
                Camera::new(
                    v3(&v["eye"]).ok_or("eye")?,
                    v3(&v["target"]).ok_or("target")?,
                    v3(&v["up"]).ok_or("up")?,
                    fov,
                ),
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok((Views { list }, w, h))
}

pub fn run(args: &[String]) -> Result<(), String> {
    let replay = arg(args, "--replay").ok_or("--replay <url> is required")?;
    let port: u16 = arg(args, "--port")
        .unwrap_or("8720")
        .parse()
        .map_err(|_| "--port")?;
    let speed: f64 = arg(args, "--speed")
        .unwrap_or("60")
        .parse()
        .map_err(|_| "--speed")?;
    let qp: u32 = arg(args, "--qp")
        .unwrap_or("18")
        .parse()
        .map_err(|_| "--qp")?;
    let fps: u32 = arg(args, "--fps")
        .unwrap_or("60")
        .parse()
        .map_err(|_| "--fps")?;
    let exit_after_end: Option<f64> = arg(args, "--exit-after-end")
        .map(|s| s.parse().map_err(|_| "--exit-after-end"))
        .transpose()?;
    let (views, w, h) =
        load_views(arg(args, "--viewpoints").unwrap_or("bench/viewpoints/flight-01.json"))?;
    let start_view = arg(args, "--view").unwrap_or("overview_sw");
    let mut camera = views
        .list
        .iter()
        .find(|(n, _)| n == start_view)
        .map(|(_, c)| c.clone())
        .ok_or_else(|| format!("unknown viewpoint {start_view}"))?;

    let (device, queue, info) =
        pointblitz_core::headless::device().ok_or("no GPU adapter available")?;
    eprintln!("adapter: {} ({:?})", info.name, info.backend);
    let mut hl = Headless::new(&device, [w, h]);
    let mut scene = Scene::new();
    let mut encoder = Encoder::new(Config {
        width: w,
        height: h,
        fps,
        qp,
    })?;

    // WebSocket clients (127.0.0.1 only, like the replay server).
    let clients: Clients = Arc::new(Mutex::new(Vec::new()));
    let joined = Arc::new(AtomicBool::new(false));
    let (input_tx, input_rx) = mpsc::channel::<(u64, Input)>();
    let listener =
        TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("bind {port}: {e}"))?;
    eprintln!("serve: ws://127.0.0.1:{port}, {w}x{h} @ {fps} fps, QP {qp}, following {replay}");
    {
        let clients = clients.clone();
        let joined = joined.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (tx, rx) = mpsc::channel();
                clients.lock().unwrap_or_else(|e| e.into_inner()).push(tx);
                joined.store(true, Ordering::SeqCst);
                let inputs = input_tx.clone();
                std::thread::spawn(move || client_thread(stream, rx, inputs));
            }
        });
    }

    if args.iter().any(|a| a == "--wait-for-client") {
        eprintln!("serve: waiting for a client");
        while clients.lock().unwrap_or_else(|e| e.into_inner()).is_empty() {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    // Replay follower: same client as the native viewer (decision 0026).
    let (msg_tx, msg_rx) = mpsc::channel::<Msg>();
    let host = client::host_port(replay)?;
    let msg_tx = Mutex::new(msg_tx);
    client::spawn_replay(host, speed, move |m| {
        let _ = msg_tx.lock().unwrap_or_else(|e| e.into_inner()).send(m);
    });

    let t0 = Instant::now();
    let tick = Duration::from_secs_f64(1.0 / f64::from(fps));
    let mut next = Instant::now();
    let mut frame = 0u64;
    let mut last_input = 0u64;
    let mut newest_visible: Option<u32> = None;
    let mut ended_at: Option<Instant> = None;
    let mut late = 0u64;
    loop {
        let tick_start = Instant::now();
        let mut visible = Vec::new();
        while let Ok(m) = msg_rx.try_recv() {
            match m {
                Msg::Chunk {
                    seq, bytes, last, ..
                } => {
                    match scene.insert(&device, &bytes) {
                        Ok(Inserted::Stale) => eprintln!("snapshot {seq}: stale chunk"),
                        Ok(_) => {}
                        Err(e) => eprintln!("snapshot {seq}: bad chunk: {e}"),
                    }
                    if last {
                        visible.push(seq);
                        newest_visible = Some(seq);
                    }
                }
                Msg::End => ended_at = Some(Instant::now()),
                Msg::Error(e) => eprintln!("replay: {e}"),
                Msg::Received { .. } | Msg::Delivered { .. } => {}
            }
        }
        while let Ok((id, input)) = input_rx.try_recv() {
            apply(&mut camera, &input, &views.list);
            last_input = last_input.max(id);
        }

        let t_render = Instant::now();
        let rgba = hl.capture(&device, &queue, &camera, &scene);
        let render_ms = t_render.elapsed().as_secs_f64() * 1e3;
        let force_idr = joined.swap(false, Ordering::SeqCst);
        let f = encoder.encode(&rgba, force_idr)?;
        let mut meta = String::new();
        write!(
            meta,
            "{{\"frame\":{frame},\"idr\":{},\"t_ms\":{:.3},\"render_ms\":{render_ms:.3},\"encode_ms\":{:.3},\"visible\":{:?},\"snapshot\":{},\"input\":{last_input},\"points\":{}}}",
            f.idr,
            tick_start.duration_since(t0).as_secs_f64() * 1e3,
            f.upload_ms + f.encode_ms,
            visible,
            newest_visible.map_or("null".into(), |s| s.to_string()),
            scene.points()
        )
        .unwrap();
        let message = Arc::new(frame_message(&meta, &f.bytes));
        clients
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|c| c.send(message.clone()).is_ok());
        frame += 1;

        if let (Some(at), Some(secs)) = (ended_at, exit_after_end)
            && at.elapsed().as_secs_f64() >= secs
        {
            eprintln!("serve: replay ended, {frame} frames, {late} late ticks");
            return Ok(());
        }
        next += tick;
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            late += 1;
            next = now; // do not try to catch up with a burst
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_inputs() {
        assert_eq!(
            parse_input(r#"{"id":3,"type":"orbit","dx":2.5,"dy":-1}"#),
            Some((3, Input::Orbit { dx: 2.5, dy: -1.0 }))
        );
        assert_eq!(
            parse_input(r#"{"id":4,"type":"view","name":"north"}"#),
            Some((
                4,
                Input::View {
                    name: "north".into()
                }
            ))
        );
        assert_eq!(parse_input(r#"{"id":5,"type":"jump"}"#), None);
        assert_eq!(parse_input("not json"), None);
    }

    #[test]
    fn frame_message_layout() {
        let m = frame_message("{\"frame\":1}", &[0, 0, 0, 1, 9]);
        assert_eq!(&m[..4], &11u32.to_le_bytes());
        assert_eq!(&m[4..15], b"{\"frame\":1}");
        assert_eq!(&m[15..], &[0, 0, 0, 1, 9]);
    }

    #[test]
    fn orbit_keeps_the_distance_and_zoom_scales_it() {
        let mut c = Camera::new([10.0, 0.0, 0.0], [0.0; 3], [0.0, 0.0, 1.0], 50.0);
        apply(&mut c, &Input::Orbit { dx: 100.0, dy: 0.0 }, &[]);
        assert!(((c.eye - c.target).length() - 10.0).abs() < 1e-9);
        apply(&mut c, &Input::Zoom { steps: 1.0 }, &[]);
        assert!(((c.eye - c.target).length() - 9.0).abs() < 1e-9);
    }
}
