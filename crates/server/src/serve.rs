//! `serve` (P3.2, decision 0036): follow the replay server, render and encode at a fixed rate,
//! stream frames over WebSocket, take camera input back.
//!
//! ```text
//! pointblitz-server serve --replay http://127.0.0.1:8700 [--port 8720] [--speed 60] [--qp 18]
//!                         [--fps 60] [--viewpoints bench/viewpoints/flight-01.json] [--view overview_sw]
//!                         [--exit-after-end <s>] [--wait-for-client] [--cold | --orbit] [--log <file>] [--wait-before-exit]
//!                         [--phase-lock on|off] [--send poll] [--data <dir>]
//! ```
//!
//! `--data <dir>` (decision 0050): the replay server's data directory is on this machine — the
//! snapshot PLYs are read there and converted in this process instead of fetched from `/chunks`.
//! Announcements still come from the replay server's `/events`.
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
//! The server also sends text messages `{"type":"snapshot","seq","kind"}` the moment a snapshot
//! is announced to it, so clients can stamp `snapshot_received` (P3.3).
//!
//! Clients send text messages: `{"id":n,"type":"orbit","dx":px,"dy":px}`, `{"id":n,"type":"zoom",
//! "steps":s}`, `{"id":n,"type":"view","name":"north"}`. A client joining forces an IDR.
//! A client may also report its display phase, `{"type":"phase","err_ms":e,"period_ms":p}`;
//! with `--phase-lock on` the tick follows the first reporting client (decision 0042; off by default).

use crate::nvenc::{Config, Encoder};
use crate::phase::PhaseLock;
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

/// Parses a client phase report (decision 0042) into (err_ms, period_ms).
pub fn parse_phase(text: &str) -> Option<(f64, f64)> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    if v["type"].as_str()? != "phase" {
        return None;
    }
    Some((v["err_ms"].as_f64()?, v["period_ms"].as_f64()?))
}

/// The orbit scenario by time, not by frame (decision 0045): wait 0.5 s after the last snapshot,
/// then 12 s at 90° per second → (wait frames, orbit frames, degrees per frame). At 60 fps this is
/// the 30 / 720 / 1.5° every earlier measurement used; at 120 fps the same path in the same time.
pub fn orbit_plan(fps: u32) -> (u64, u64, f64) {
    (
        u64::from(fps) / 2,
        12 * u64::from(fps),
        90.0 / f64::from(fps),
    )
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

/// What goes to a client: a frame (binary) or an announcement (text).
#[derive(Clone)]
enum Out {
    Frame(Arc<Vec<u8>>),
    Text(Arc<String>),
}

type Clients = Arc<Mutex<Vec<mpsc::Sender<Out>>>>;

fn broadcast(clients: &Clients, out: &Out) {
    clients
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|c| c.send(out.clone()).is_ok());
}

fn to_message(out: Out) -> Message {
    match out {
        Out::Frame(f) => Message::Binary(f.as_ref().clone().into()),
        Out::Text(t) => Message::Text(t.as_str().into()),
    }
}

/// One client. `poll` = false (default, decision 0042): a writer thread sends each frame the moment
/// it is queued and this thread blocks reading input. `poll` = true: the earlier single thread that
/// read with a 1 ms timeout and sent in between — on Windows that timeout waits for the 15.6 ms
/// system timer tick, so frames left on a 15.6 ms grid instead of every 16.7 ms (a sawtooth delay
/// that walks through the client's display cycle; kept for the A/B only).
fn client_thread(
    stream: TcpStream,
    frames: mpsc::Receiver<Out>,
    inputs: mpsc::Sender<(u64, Input, Instant)>,
    client: usize,
    phases: mpsc::Sender<(usize, f64, f64)>,
    poll: bool,
) {
    let _ = stream.set_nodelay(true);
    let Ok(mut ws) = tungstenite::accept(stream) else {
        return;
    };
    let on_text = |t: &str| {
        if let Some(i) = parse_input(t) {
            let _ = inputs.send((i.0, i.1, Instant::now()));
        } else if let Some((err, period)) = parse_phase(t) {
            let _ = phases.send((client, err, period));
        }
    };
    if !poll {
        // The reader never writes (clients send no pings), so the two WebSocket objects on one
        // socket do not interleave writes. Every frame is sent: P frames depend on all earlier ones.
        let Ok(raw) = ws.get_ref().try_clone() else {
            return;
        };
        let mut writer =
            tungstenite::WebSocket::from_raw_socket(raw, tungstenite::protocol::Role::Server, None);
        std::thread::spawn(move || {
            for out in frames {
                if writer.send(to_message(out)).is_err() {
                    return;
                }
            }
        });
        loop {
            match ws.read() {
                Ok(Message::Text(t)) => on_text(t.as_str()),
                Ok(Message::Close(_)) | Err(_) => return,
                Ok(_) => {}
            }
        }
    }
    let _ = ws
        .get_mut()
        .set_read_timeout(Some(Duration::from_millis(1)));
    loop {
        match ws.read() {
            Ok(Message::Text(t)) => on_text(t.as_str()),
            Ok(Message::Close(_)) => return,
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return,
        }
        while let Ok(out) = frames.try_recv() {
            if ws.send(to_message(out)).is_err() {
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
    // --cold: only the last snapshot, as one full delivery (SPEC §6.1 cold).
    // --orbit (P3.4): the last snapshot, then three turns around the start view's target at 1.5° per
    // frame; frame metadata says phase load / orbit / done, and the server exits shortly after.
    let orbit = args.iter().any(|a| a == "--orbit");
    let cold = orbit || args.iter().any(|a| a == "--cold");
    // --log <file>: per input (received → applied tick → frame) and per late tick (where the time
    // went), as JSON lines — for the P3.4 analysis.
    let log_path = arg(args, "--log").map(str::to_string);
    let data_dir = arg(args, "--data").map(std::path::PathBuf::from);
    // --wait-before-exit: when finished, print "finished" on stderr and wait for a line on stdin, so
    // a harness can read this process's OS peak memory while it exists (decision 0032).
    let wait_before_exit = args.iter().any(|a| a == "--wait-before-exit");
    // --phase-lock on|off (decision 0042): move the tick so frames reach the first reporting client
    // in the middle of its display cycle. Off by default: in the C1–C4 A/B one orbit run of five went
    // over 1 % (all its empty cycles while the lock was still converging) and refined latency was
    // higher, so the acceptance bound was not met.
    let phase_lock = match arg(args, "--phase-lock").unwrap_or("off") {
        "on" => true,
        "off" => false,
        _ => return Err("--phase-lock on|off".into()),
    };
    // --send poll: the earlier polling send path (A/B only, see client_thread).
    let poll_send = arg(args, "--send") == Some("poll");
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
    let (input_tx, input_rx) = mpsc::channel::<(u64, Input, Instant)>();
    let (phase_tx, phase_rx) = mpsc::channel::<(usize, f64, f64)>();
    let listener =
        TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("bind {port}: {e}"))?;
    eprintln!(
        "serve: ws://127.0.0.1:{port}, {w}x{h} @ {fps} fps, QP {qp}, phase lock {}, following {replay}",
        if phase_lock { "on" } else { "off" }
    );
    {
        let clients = clients.clone();
        let joined = joined.clone();
        std::thread::spawn(move || {
            for (client, stream) in listener.incoming().flatten().enumerate() {
                let (tx, rx) = mpsc::channel();
                clients.lock().unwrap_or_else(|e| e.into_inner()).push(tx);
                joined.store(true, Ordering::SeqCst);
                let inputs = input_tx.clone();
                let phases = phase_tx.clone();
                std::thread::spawn(move || {
                    client_thread(stream, rx, inputs, client, phases, poll_send)
                });
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
    let send = move |m| {
        let _ = msg_tx.lock().unwrap_or_else(|e| e.into_inner()).send(m);
    };
    match (data_dir, cold) {
        (Some(dir), true) => client::spawn_cold_local(host, dir, send),
        (Some(dir), false) => client::spawn_replay_local(host, dir, speed, send),
        (None, true) => client::spawn_cold(host, send),
        (None, false) => client::spawn_replay(host, speed, send),
    }

    let t0 = Instant::now();
    let tick = Duration::from_secs_f64(1.0 / f64::from(fps));
    let mut next = Instant::now();
    let mut frame = 0u64;
    let mut last_input = 0u64;
    let mut newest_visible: Option<u32> = None;
    let mut ended_at: Option<Instant> = None;
    let mut late = 0u64;
    let mut log = String::new();
    let (mut render_all, mut encode_all, mut over_budget) = (Vec::new(), Vec::new(), 0u64);
    let (orbit_wait, orbit_frames, orbit_step_deg) = orbit_plan(fps);
    let mut orbit_from: Option<u64> = None;
    let orbit_base = camera.clone();
    let ms_since = |t: Instant| t.duration_since(t0).as_secs_f64() * 1e3;
    let mut lock = PhaseLock::default();
    // The client the lock follows: the first that reports; another takes over once it goes quiet.
    let mut lock_client: Option<(usize, Instant)> = None;
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
                // Forward the announcement at once: a client stamps snapshot_received when this
                // arrives, so event_latency starts where the event reaches the server (P3.3).
                Msg::Received { snap, .. } => broadcast(
                    &clients,
                    &Out::Text(Arc::new(format!(
                        r#"{{"type":"snapshot","seq":{},"kind":"{}"}}"#,
                        snap.seq, snap.kind
                    ))),
                ),
                Msg::Delivered { .. } => {}
            }
        }
        let t_msgs = Instant::now();
        while let Ok((id, input, rx)) = input_rx.try_recv() {
            apply(&mut camera, &input, &views.list);
            last_input = last_input.max(id);
            writeln!(
                log,
                r#"{{"type":"input","id":{id},"rx_ms":{:.3},"tick_ms":{:.3},"frame":{frame}}}"#,
                ms_since(rx),
                ms_since(tick_start)
            )
            .unwrap();
        }
        while let Ok((client, err, period)) = phase_rx.try_recv() {
            let now = Instant::now();
            if lock_client
                .is_some_and(|(c, at)| c != client && now.duration_since(at).as_secs_f64() < 2.0)
            {
                continue;
            }
            lock_client = Some((client, now));
            if !phase_lock {
                continue;
            }
            if let Some(r) = lock.report(err, period, ms_since(now)) {
                writeln!(
                    log,
                    r#"{{"type":"phase","client":{client},"t_ms":{:.3},"err_ms":{:.3},"added_ms":{:.3},"integral_ms":{:.3}}}"#,
                    ms_since(now),
                    r.err_ms,
                    r.added_ms,
                    r.integral_ms
                )
                .unwrap();
            }
        }
        let phase = if orbit {
            if orbit_from.is_none() && ended_at.is_some() && newest_visible.is_some() {
                orbit_from = Some(frame + orbit_wait);
            }
            match orbit_from {
                Some(f0) if frame >= f0 && frame < f0 + orbit_frames => {
                    let k = (frame - f0) as f64 * orbit_step_deg;
                    let up = orbit_base.up.normalize();
                    camera.eye = orbit_base.target
                        + rotate(orbit_base.eye - orbit_base.target, up, k.to_radians());
                    "orbit"
                }
                Some(f0) if frame >= f0 + orbit_frames => "done",
                _ => "load",
            }
        } else {
            "replay"
        };

        let t_render = Instant::now();
        let rgba = hl.capture(&device, &queue, &camera, &scene);
        let render_ms = t_render.elapsed().as_secs_f64() * 1e3;
        let force_idr = joined.swap(false, Ordering::SeqCst);
        let f = encoder.encode(&rgba, force_idr)?;
        let mut meta = String::new();
        write!(
            meta,
            "{{\"frame\":{frame},\"idr\":{},\"t_ms\":{:.3},\"render_ms\":{render_ms:.3},\"encode_ms\":{:.3},\"visible\":{:?},\"snapshot\":{},\"input\":{last_input},\"points\":{},\"phase\":\"{phase}\"}}",
            f.idr,
            tick_start.duration_since(t0).as_secs_f64() * 1e3,
            f.upload_ms + f.encode_ms,
            visible,
            newest_visible.map_or("null".into(), |s| s.to_string()),
            scene.points()
        )
        .unwrap();
        broadcast(
            &clients,
            &Out::Frame(Arc::new(frame_message(&meta, &f.bytes))),
        );
        frame += 1;
        let tick_ms = tick_start.elapsed().as_secs_f64() * 1e3;
        render_all.push(render_ms);
        encode_all.push(f.upload_ms + f.encode_ms);
        if tick_ms > 1000.0 / f64::from(fps) {
            over_budget += 1;
            writeln!(
                log,
                r#"{{"type":"late","frame":{},"tick_ms":{tick_ms:.3},"messages_ms":{:.3},"render_ms":{render_ms:.3},"encode_ms":{:.3},"chunks":{}}}"#,
                frame - 1,
                t_msgs.duration_since(tick_start).as_secs_f64() * 1e3,
                f.upload_ms + f.encode_ms,
                visible.len()
            )
            .unwrap();
        }

        let finished = if orbit {
            orbit_from.is_some_and(|f0| frame >= f0 + orbit_frames + orbit_wait)
        } else {
            matches!((ended_at, exit_after_end), (Some(at), Some(secs)) if at.elapsed().as_secs_f64() >= secs)
        };
        if finished {
            let pct = |v: &mut Vec<f64>, q: f64| {
                v.sort_by(f64::total_cmp);
                v.get(((v.len().max(1) - 1) as f64 * q).round() as usize)
                    .copied()
                    .unwrap_or(f64::NAN)
            };
            writeln!(
                log,
                r#"{{"type":"summary","frames":{frame},"late_ticks":{late},"over_budget_ticks":{over_budget},"render_ms_p50":{:.3},"render_ms_p95":{:.3},"render_ms_p99":{:.3},"encode_ms_p50":{:.3},"encode_ms_p95":{:.3},"encode_ms_p99":{:.3}}}"#,
                pct(&mut render_all, 0.5),
                pct(&mut render_all, 0.95),
                pct(&mut render_all, 0.99),
                pct(&mut encode_all, 0.5),
                pct(&mut encode_all, 0.95),
                pct(&mut encode_all, 0.99),
            )
            .unwrap();
            eprintln!("serve: finished, {frame} frames, {late} late ticks");
            if let Some(path) = &log_path {
                std::fs::write(path, &log).map_err(|e| format!("{path}: {e}"))?;
            }
            if wait_before_exit {
                eprintln!("finished");
                let mut line = String::new();
                let _ = std::io::stdin().read_line(&mut line);
            }
            return Ok(());
        }
        next += tick;
        // Phase lock: at most ±1 ms per tick, later (positive) or earlier (negative).
        let shift = lock.tick_shift(ms_since(Instant::now()));
        if shift >= 0.0 {
            next += Duration::from_secs_f64(shift / 1e3);
        } else {
            next -= Duration::from_secs_f64(-shift / 1e3);
        }
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
    fn orbit_plan_keeps_the_60_fps_scenario() {
        assert_eq!(orbit_plan(60), (30, 720, 1.5));
        let (w, n, step) = orbit_plan(120);
        assert_eq!((w, n), (60, 1440));
        assert!((step * n as f64 - 1080.0).abs() < 1e-9); // three turns either way
    }

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
