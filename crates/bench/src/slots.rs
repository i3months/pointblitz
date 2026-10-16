//! `replay --stream <dir>` (decision 0052): replays a cumulus3d v4 slot diff stream
//! (`<out>/live/stream`) at its original event times, like the snapshot replay (decision 0018).
//!
//! The packs are read and validated once at start-up; a request only puts the 32 B header in front
//! (`pointblitz_io::slots`), so there is no conversion on the delivery path (decision 0050).
//!
//! Endpoints (bound to 127.0.0.1 only):
//! - `GET /manifest.json` — `{"stream":"slots","version":4,"frame","origin_gps","capacity","events":[…]}`;
//!   per event `seq`, `time_s` (= `t`), `kind`, `zone`, `points`, `n_add`, `n_move`, `n_remove`,
//!   `bytes` (pack), `capacity` (after the event) and `url`.
//! - `GET /events?speed=N&start=first|zero` — SSE stream, one `event: slots` per event with the same
//!   JSON, then `event: end`.
//! - `GET /slots/<seq>` — that event's message (header + pack).
//! - `GET /slots/state?seq=N` — every message through `seq` N folded into one (a client joining
//!   late starts from it). Without `seq`: through the newest event announced on any `/events`
//!   stream so far, or the whole stream when nothing was announced (cold start).
//! - `GET /static/<path>` — files under the web root.

use crate::replay::{parse_request_line, respond, safe_join, schedule_times, serve_file};
use pointblitz_io::slots::dir::{Manifest, StreamDir, StreamEvent};
use pointblitz_io::slots::{Fold, validate};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

struct Server {
    web: Option<PathBuf>,
    manifest: Manifest,
    /// Per event (manifest order): its message.
    messages: Vec<Arc<Vec<u8>>>,
    /// Newest seq announced on any `/events` stream; 0 before the first.
    announced: AtomicU32,
    /// The last folded state served: (seq, message).
    state: Mutex<Option<(u32, Arc<Vec<u8>>)>>,
}

impl Server {
    fn load(dir: PathBuf, web: Option<PathBuf>) -> Result<Self, String> {
        let mut s = StreamDir::open(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let messages = s
            .messages()
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .into_iter()
            .map(|(_, m)| Arc::new(m))
            .collect();
        Ok(Self {
            web,
            manifest: s.manifest().clone(),
            messages,
            announced: AtomicU32::new(0),
            state: Mutex::new(None),
        })
    }

    fn index(&self, seq: u32) -> Option<usize> {
        self.manifest.events.iter().position(|e| e.seq == seq)
    }

    /// Everything through event `idx`, folded (the latest one is kept).
    fn state(&self, idx: usize) -> Arc<Vec<u8>> {
        let seq = self.manifest.events[idx].seq;
        let mut held = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((s, m)) = held.as_ref()
            && *s == seq
        {
            return Arc::clone(m);
        }
        let mut fold = Fold::new();
        for m in &self.messages[..=idx] {
            fold.apply(m).expect("validated at start-up");
        }
        let m = Arc::new(fold.message());
        *held = Some((seq, Arc::clone(&m)));
        m
    }
}

/// One event as JSON (manifest entry and SSE data).
pub fn event_json(e: &StreamEvent, capacity: u32) -> String {
    format!(
        "{{\"seq\":{},\"time_s\":{},\"t\":{},\"kind\":\"{}\",\"zone\":{},\"points\":{},\"n_add\":{},\"n_move\":{},\"n_remove\":{},\"bytes\":{},\"capacity\":{capacity},\"url\":\"/slots/{}\"}}",
        e.seq, e.t, e.t, e.kind, e.zone, e.points, e.n_add, e.n_move, e.n_remove, e.bytes, e.seq
    )
}

fn capacity_of(m: &[u8]) -> u32 {
    validate(m).map_or(0, |h| h.capacity)
}

fn manifest_json(server: &Server) -> String {
    let m = &server.manifest;
    let events: Vec<String> = m
        .events
        .iter()
        .zip(&server.messages)
        .map(|(e, msg)| event_json(e, capacity_of(msg)))
        .collect();
    let origin = m
        .origin_gps
        .map_or("null".into(), |o| format!("[{},{},{}]", o[0], o[1], o[2]));
    format!(
        "{{\"stream\":\"slots\",\"version\":{},\"frame\":\"{}\",\"origin_gps\":{origin},\"capacity\":{},\"events\":[{}]}}",
        m.version,
        m.frame,
        m.capacity,
        events.join(",")
    )
}

pub fn run(dir: PathBuf, web: Option<PathBuf>, port: u16) -> Result<(), String> {
    let t0 = Instant::now();
    let server = Arc::new(Server::load(dir, web)?);
    let bytes: usize = server.messages.iter().map(|m| m.len()).sum();
    let listener =
        TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("bind {port}: {e}"))?;
    eprintln!(
        "replay: slot stream, {} events, {bytes} B of messages (loaded in {:.0} ms), http://127.0.0.1:{port}/manifest.json",
        server.manifest.events.len(),
        t0.elapsed().as_secs_f64() * 1e3
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
            manifest_json(server).as_bytes(),
        ),
        "/events" => {
            let speed = q("speed").and_then(|s| s.parse().ok()).unwrap_or(1.0f64);
            let start_first = q("start") != Some("zero");
            stream_events(server, &mut stream, speed.max(1e-6), start_first)
        }
        "/slots/state" => {
            let seq = match q("seq") {
                Some(s) => s.parse().ok(),
                None => match server.announced.load(Ordering::SeqCst) {
                    0 => server.manifest.events.last().map(|e| e.seq),
                    s => Some(s),
                },
            };
            match seq.and_then(|s| server.index(s)) {
                Some(idx) => {
                    let t = Instant::now();
                    let m = server.state(idx);
                    eprintln!(
                        "slots: state through seq {} -> {} B, {:.1} ms",
                        server.manifest.events[idx].seq,
                        m.len(),
                        t.elapsed().as_secs_f64() * 1e3
                    );
                    send_message(&mut stream, &m)
                }
                None => respond(&mut stream, 404, "text/plain", b"not found"),
            }
        }
        p if p.starts_with("/slots/") => {
            let seq: Option<u32> = p["/slots/".len()..].parse().ok();
            match seq.and_then(|s| server.index(s)) {
                Some(idx) => send_message(&mut stream, &server.messages[idx]),
                None => respond(&mut stream, 404, "text/plain", b"not found"),
            }
        }
        "/favicon.ico" => respond(&mut stream, 204, "text/plain", b""),
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

fn send_message(stream: &mut TcpStream, m: &[u8]) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/x-pointblitz-slots\r\n\
         Content-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        m.len()
    )?;
    stream.write_all(m)?;
    stream.flush()
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
    let times: Vec<f64> = server.manifest.events.iter().map(|e| e.t).collect();
    let t0 = Instant::now();
    for ((e, msg), at) in server
        .manifest
        .events
        .iter()
        .zip(&server.messages)
        .zip(schedule_times(&times, speed, start_first))
    {
        if let Some(wait) = at.checked_sub(t0.elapsed()) {
            std::thread::sleep(wait);
        }
        server.announced.fetch_max(e.seq, Ordering::SeqCst);
        let json = event_json(e, capacity_of(msg));
        write!(stream, "event: slots\ndata: {json}\n\n")?;
        stream.flush()?;
        let unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        println!("{{\"sent_unix_ms\":{unix_ms},\"speed\":{speed},\"event\":{json}}}");
    }
    write!(stream, "event: end\ndata: {{}}\n\n")?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pointblitz_io::slots::{Diff, Record, SlotHeader, encode};

    fn pack(adds: &[u32], moves: &[u32], removes: &[u32]) -> (Vec<u8>, [u32; 3]) {
        let mut p = Vec::new();
        for (ids, zone) in [(adds, 2), (moves, 0)] {
            for &id in ids {
                Record {
                    id,
                    position: [id as f32, 0.0, 1.0],
                    color: [1, 2, 3],
                    zone,
                }
                .write(&mut p);
            }
        }
        for id in removes {
            p.extend(id.to_le_bytes());
        }
        (
            p,
            [adds.len(), moves.len(), removes.len()].map(|n| n as u32),
        )
    }

    #[test]
    fn serves_messages_and_folded_states() {
        let dir = std::env::temp_dir().join(format!("pb-slot-replay-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("packs")).unwrap();
        let events = [pack(&[0, 1, 2], &[], &[]), pack(&[3], &[1], &[0])];
        let mut items = Vec::new();
        let mut points = 0i64;
        for (k, (p, n)) in events.iter().enumerate() {
            let seq = k + 1;
            std::fs::write(dir.join(format!("packs/e_{seq:04}.bin")), p).unwrap();
            points += i64::from(n[0]) - i64::from(n[2]);
            items.push(format!(
                r#"{{"seq":{seq},"t":{}.0,"zone":0,"kind":"refined","points":{points},"pack":"packs/e_{seq:04}.bin","n_add":{},"n_move":{},"n_remove":{},"bytes":{}}}"#,
                10 * seq,
                n[0],
                n[1],
                n[2],
                p.len()
            ));
        }
        std::fs::write(
            dir.join("manifest.json"),
            format!(
                r#"{{"version":4,"frame":"enu","origin_gps":[1.5,2.5,3.5],"capacity":4,"events":[{}]}}"#,
                items.join(",")
            ),
        )
        .unwrap();
        let server = Server::load(dir.clone(), None).unwrap();
        // The message is the header + the pack unchanged.
        let h = SlotHeader {
            seq: 2,
            n_add: 1,
            n_move: 1,
            n_remove: 1,
            capacity: 4,
        };
        assert_eq!(*server.messages[1], encode(&h, &events[1].0).unwrap());
        let json = manifest_json(&server);
        assert!(json.starts_with(r#"{"stream":"slots","version":4,"frame":"enu","origin_gps":[1.5,2.5,3.5],"capacity":4,"events":[{"seq":1,"time_s":10,"#));
        assert!(json.contains(r#""capacity":3,"url":"/slots/1""#));
        // State through 1 = event 1; through 2 = slots 1, 2, 3.
        let s1 = server.state(0);
        assert_eq!(*s1, *server.messages[0]);
        let s2 = server.state(1);
        let ids: Vec<u32> = Diff::parse(&s2)
            .unwrap()
            .add_records()
            .map(|r| r.id)
            .collect();
        assert_eq!(ids, vec![1, 2, 3]);
        assert!(
            Arc::ptr_eq(&s2, &server.state(1)),
            "kept for the next request"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
