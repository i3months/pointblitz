//! Scenario replay server (decision 0018).
//!
//! Serves a dataset of snapshot PLY files and announces each snapshot over Server-Sent Events at
//! its original time, optionally sped up. The same server feeds the three.js baseline and the
//! PointBlitz clients, so every implementation sees identical bytes and timing.
//!
//! Endpoints (bound to 127.0.0.1 only):
//! - `GET /manifest.json` — the event list.
//! - `GET /events?speed=N&start=first|zero` — SSE stream, one `event` per snapshot.
//! - `GET /data/<file>` — snapshot bytes, uncompressed.
//! - `GET /chunks/<seq>?have=<generation>.<points>` — the snapshot as PointBlitz chunks (`chunks.rs`).
//! - `GET /static/<path>` — files under the web root (baseline pages).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// One snapshot of the dataset.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub seq: u32,
    /// Seconds since the start of the flight, from the file name.
    pub time_s: f64,
    pub kind: Kind,
    /// Region index from the file name (`preview_3` → 3).
    pub region: u32,
    pub file: String,
    pub bytes: u64,
    pub points: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Preview,
    Refined,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Refined => "refined",
        }
    }
}

/// Parses `event_<seq>_<time>s_<kind>_<region>.ply`.
pub fn parse_name(name: &str) -> Option<(u32, f64, Kind, u32)> {
    let stem = name.strip_suffix(".ply")?.strip_prefix("event_")?;
    let mut parts = stem.split('_');
    let seq = parts.next()?.parse().ok()?;
    let time_s = parts.next()?.strip_suffix('s')?.parse().ok()?;
    let kind = match parts.next()? {
        "preview" => Kind::Preview,
        "refined" => Kind::Refined,
        _ => return None,
    };
    let region = parts.next()?.parse().ok()?;
    parts
        .next()
        .is_none()
        .then_some((seq, time_s, kind, region))
}

/// Reads the dataset directory into an ordered event list.
pub fn scan(dir: &Path) -> Result<Vec<Event>, String> {
    let mut events = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((seq, time_s, kind, region)) = parse_name(&name) else {
            continue;
        };
        let path = entry.path();
        let bytes = entry.metadata().map_err(|e| e.to_string())?.len();
        let mut head = vec![0u8; 4096.min(bytes as usize)];
        std::fs::File::open(&path)
            .and_then(|mut f| f.read_exact(&mut head))
            .map_err(|e| format!("{name}: {e}"))?;
        let points = pointblitz_io::parse_header(&head)
            .map_err(|e| format!("{name}: {e}"))?
            .vertex_count;
        events.push(Event {
            seq,
            time_s,
            kind,
            region,
            file: name,
            bytes,
            points,
        });
    }
    if events.is_empty() {
        return Err(format!("no event_*.ply files in {}", dir.display()));
    }
    events.sort_by(|a, b| a.time_s.total_cmp(&b.time_s).then(a.seq.cmp(&b.seq)));
    Ok(events)
}

/// When each event is sent, relative to the moment the client subscribes.
pub fn schedule(events: &[Event], speed: f64, start_at_first: bool) -> Vec<Duration> {
    let origin = if start_at_first {
        events.first().map_or(0.0, |e| e.time_s)
    } else {
        0.0
    };
    events
        .iter()
        .map(|e| Duration::from_secs_f64(((e.time_s - origin) / speed).max(0.0)))
        .collect()
}

pub fn manifest_json(events: &[Event]) -> String {
    let items: Vec<String> = events.iter().map(event_json).collect();
    format!("{{\"events\":[{}]}}", items.join(","))
}

fn event_json(e: &Event) -> String {
    format!(
        "{{\"seq\":{},\"time_s\":{},\"kind\":\"{}\",\"region\":{},\"file\":\"{}\",\"url\":\"/data/{}\",\"bytes\":{},\"points\":{}}}",
        e.seq,
        e.time_s,
        e.kind.as_str(),
        e.region,
        e.file,
        e.file,
        e.bytes,
        e.points
    )
}

/// A parsed request line.
#[derive(Debug, PartialEq)]
pub struct Request {
    pub path: String,
    pub query: Vec<(String, String)>,
}

pub fn parse_request_line(line: &str) -> Option<Request> {
    let mut it = line.split_whitespace();
    if it.next()? != "GET" {
        return None;
    }
    let target = it.next()?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let query = query
        .split('&')
        .filter(|kv| !kv.is_empty())
        .map(|kv| {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            (k.to_string(), v.to_string())
        })
        .collect();
    Some(Request {
        path: path.to_string(),
        query,
    })
}

/// Joins a URL path below `root`, refusing anything that would escape it.
/// Hidden entries (`.git`, `.github`, …) are refused too: the web root is often the repository root.
pub fn safe_join(root: &Path, rel: &str) -> Option<PathBuf> {
    let rel = Path::new(rel);
    let hidden_or_escaping = |c: Component| match c {
        Component::Normal(name) => name.to_string_lossy().starts_with('.'),
        _ => true,
    };
    if rel.components().any(hidden_or_escaping) {
        return None;
    }
    Some(root.join(rel))
}

struct Server {
    data: PathBuf,
    web: Option<PathBuf>,
    events: Vec<Event>,
    /// `appends[k]`: snapshot k holds snapshot k − 1 unchanged at its start (crate::chunks).
    appends: Vec<bool>,
}

pub fn run(args: &[String]) -> Result<(), String> {
    let mut data = None;
    let mut web = None;
    let mut port = 8700u16;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--data" => data = it.next().map(PathBuf::from),
            "--web" => web = it.next().map(PathBuf::from),
            "--port" => {
                port = it
                    .next()
                    .and_then(|p| p.parse().ok())
                    .ok_or("--port needs a number")?
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let data = data.ok_or("--data <dir> is required")?;
    let events = scan(&data)?;
    // Checked once on the bytes at startup and treated as snapshot metadata (decision 0026): the
    // producer knows whether it appended, so this is not part of the per-request work.
    let mut appends = vec![false; events.len()];
    let mut prev: Option<Vec<u8>> = None;
    for (k, e) in events.iter().enumerate() {
        let path = data.join(&e.file);
        let bytes = std::fs::read(&path).map_err(|err| format!("{}: {err}", path.display()))?;
        if let Some(p) = &prev {
            appends[k] = pointblitz_io::is_prefix(p, &bytes).unwrap_or(false);
        }
        prev = Some(bytes);
    }
    let server = Arc::new(Server {
        data,
        web,
        events,
        appends,
    });
    let listener =
        TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("bind {port}: {e}"))?;
    eprintln!(
        "replay: {} events, http://127.0.0.1:{port}/manifest.json",
        server.events.len()
    );
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let server = Arc::clone(&server);
        std::thread::spawn(move || {
            if let Err(e) = handle(&server, stream) {
                eprintln!("replay: {e}");
            }
        });
    }
    Ok(())
}

fn handle(server: &Server, mut stream: TcpStream) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    // Drain the remaining headers.
    let mut h = String::new();
    while reader.read_line(&mut h)? > 2 {
        h.clear();
    }
    let Some(req) = parse_request_line(&line) else {
        return respond(&mut stream, 405, "text/plain", b"only GET");
    };
    let q = |k: &str| {
        req.query
            .iter()
            .find(|(qk, _)| qk == k)
            .map(|(_, v)| v.as_str())
    };
    match req.path.as_str() {
        "/manifest.json" => respond(
            &mut stream,
            200,
            "application/json",
            manifest_json(&server.events).as_bytes(),
        ),
        "/events" => {
            let speed = q("speed").and_then(|s| s.parse().ok()).unwrap_or(1.0f64);
            let start_first = q("start") != Some("zero");
            stream_events(server, &mut stream, speed.max(1e-6), start_first)
        }
        p if p.starts_with("/data/") => {
            let name = &p["/data/".len()..];
            match server.events.iter().find(|e| e.file == name) {
                Some(e) => serve_file(&mut stream, &server.data.join(&e.file)),
                None => respond(&mut stream, 404, "text/plain", b"not found"),
            }
        }
        p if p.starts_with("/chunks/") => {
            let seq: Option<u32> = p["/chunks/".len()..].parse().ok();
            match server.events.iter().position(|e| Some(e.seq) == seq) {
                Some(idx) => serve_chunks(
                    server,
                    &mut stream,
                    idx,
                    q("have").and_then(crate::chunks::parse_have),
                ),
                None => respond(&mut stream, 404, "text/plain", b"not found"),
            }
        }
        p if p.starts_with("/static/") => {
            match server
                .web
                .as_deref()
                .and_then(|w| safe_join(w, &p["/static/".len()..]))
            {
                Some(path) if path.is_file() => serve_file(&mut stream, &path),
                _ => respond(&mut stream, 404, "text/plain", b"not found"),
            }
        }
        _ => respond(&mut stream, 404, "text/plain", b"not found"),
    }
}

fn stream_events(
    server: &Server,
    stream: &mut TcpStream,
    speed: f64,
    start_first: bool,
) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\n\
         Connection: close\r\n\r\n"
    )?;
    stream.flush()?;
    let t0 = Instant::now();
    for (e, at) in server
        .events
        .iter()
        .zip(schedule(&server.events, speed, start_first))
    {
        if let Some(wait) = at.checked_sub(t0.elapsed()) {
            std::thread::sleep(wait);
        }
        let json = event_json(e);
        write!(stream, "event: snapshot\ndata: {json}\n\n")?;
        stream.flush()?;
        let unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        println!("{{\"sent_unix_ms\":{unix_ms},\"speed\":{speed},\"event\":{json}}}");
    }
    write!(stream, "event: end\ndata: {{}}\n\n")?;
    stream.flush()
}

/// Converts snapshot `idx` and streams its chunks (crate::chunks). The body has no length: the
/// client reads chunks until the one flagged last, and the connection closes.
fn serve_chunks(
    server: &Server,
    stream: &mut TcpStream,
    idx: usize,
    have: Option<(u32, usize)>,
) -> std::io::Result<()> {
    use crate::chunks::{MAX_POINTS, Plan, plan};
    let t0 = Instant::now();
    let ply = std::fs::read(server.data.join(&server.events[idx].file))?;
    let p = plan(&server.events, idx, have, |k| server.appends[k]);
    let (generation, skip, kind) = match p {
        Plan::Full { generation } => (generation, 0, "full"),
        Plan::Delta { generation, skip } => (generation, skip, "delta"),
    };
    let read_ms = t0.elapsed().as_secs_f64() * 1e3;
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/x-pointblitz-chunks\r\n\
         X-PB-Delivery: {kind}\r\nX-PB-Generation: {generation}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n"
    )?;
    let mut sent = 0u64;
    let mut write_err = None;
    let res = pointblitz_io::chunk::ply_tail_to_chunks(&ply, generation, skip, MAX_POINTS, |c| {
        match stream.write_all(&c) {
            Ok(()) => {
                sent += c.len() as u64;
                true
            }
            Err(e) => {
                write_err = Some(e);
                false
            }
        }
    });
    if let Some(e) = write_err {
        return Err(e);
    }
    let chunks = res.map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    stream.flush()?;
    eprintln!(
        "chunks: seq {} {kind} gen {generation} skip {skip} -> {chunks} chunks, {sent} B, \
         read {read_ms:.1} ms, total {:.1} ms",
        server.events[idx].seq,
        t0.elapsed().as_secs_f64() * 1e3
    );
    Ok(())
}

fn serve_file(stream: &mut TcpStream, path: &Path) -> std::io::Result<()> {
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    let ty = match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript",
        Some("json") => "application/json",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    };
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: {ty}\r\nContent-Length: {len}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n"
    )?;
    std::io::copy(&mut file, stream)?;
    stream.flush()
}

fn respond(stream: &mut TcpStream, code: u16, ty: &str, body: &[u8]) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        404 => "Not Found",
        _ => "Method Not Allowed",
    };
    write!(
        stream,
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {ty}\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_skyrecon_snapshot_names() {
        assert_eq!(
            parse_name("event_04_0327.4s_refined_1.ply"),
            Some((4, 327.4, Kind::Refined, 1))
        );
        assert_eq!(
            parse_name("event_13_2005.6s_preview_6.ply"),
            Some((13, 2005.6, Kind::Preview, 6))
        );
        assert_eq!(parse_name("event_01_0097.0s_other_0.ply"), None);
        assert_eq!(parse_name("final.ply"), None);
        assert_eq!(parse_name("event_01_0097.0s_preview_0_x.ply"), None);
    }

    fn ev(seq: u32, t: f64) -> Event {
        Event {
            seq,
            time_s: t,
            kind: Kind::Preview,
            region: 0,
            file: format!("e{seq}"),
            bytes: 0,
            points: 0,
        }
    }

    #[test]
    fn schedule_scales_and_shifts() {
        let e = [ev(1, 97.0), ev(2, 127.0), ev(3, 157.0)];
        let s = schedule(&e, 10.0, true);
        assert_eq!(
            s,
            vec![
                Duration::ZERO,
                Duration::from_secs(3),
                Duration::from_secs(6)
            ]
        );
        let z = schedule(&e, 1.0, false);
        assert_eq!(z[0], Duration::from_secs(97));
    }

    #[test]
    fn parses_request_lines() {
        assert_eq!(
            parse_request_line("GET /events?speed=60&start=zero HTTP/1.1\r\n"),
            Some(Request {
                path: "/events".into(),
                query: vec![
                    ("speed".into(), "60".into()),
                    ("start".into(), "zero".into())
                ],
            })
        );
        assert_eq!(parse_request_line("POST / HTTP/1.1"), None);
    }

    #[test]
    fn refuses_path_traversal() {
        let root = Path::new("web");
        assert_eq!(
            safe_join(root, "baseline/index.html"),
            Some(root.join("baseline/index.html"))
        );
        assert_eq!(safe_join(root, "../secret"), None);
        assert_eq!(safe_join(root, "/etc/passwd"), None);
        assert_eq!(safe_join(root, ".git/config"), None);
        assert_eq!(safe_join(root, "baseline/.hidden"), None);
    }

    #[test]
    fn manifest_lists_events_in_order() {
        let json = manifest_json(&[ev(1, 1.0), ev(2, 2.5)]);
        assert!(json.starts_with("{\"events\":[{\"seq\":1,"));
        assert!(json.contains("\"url\":\"/data/e2\""));
    }
}
