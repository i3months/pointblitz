//! `probe` (P3.2): a headless test client for `serve`.
//!
//! ```text
//! pointblitz-server probe [--url ws://127.0.0.1:8720] [--expect-snapshots 14] [--max-seconds 120]
//! ```
//! Receives frames until every expected snapshot has become visible (or time runs out), sends a
//! numbered orbit input every 60 frames, and checks: the first frame is an IDR, frame numbers are
//! contiguous, every input is acknowledged in a later frame's metadata, all snapshots were seen.
//! Prints one JSON summary (also frame intervals as received) and fails if a check fails.

use std::time::{Duration, Instant};
use tungstenite::Message;

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

/// Splits a frame message into (metadata JSON, H.264 bytes).
pub fn split_frame(m: &[u8]) -> Option<(&str, &[u8])> {
    let len = u32::from_le_bytes(m.get(..4)?.try_into().ok()?) as usize;
    let meta = std::str::from_utf8(m.get(4..4 + len)?).ok()?;
    Some((meta, &m[4 + len..]))
}

fn pct(v: &mut [f64], q: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * q).round() as usize]
}

pub fn run(args: &[String]) -> Result<(), String> {
    let url = arg(args, "--url").unwrap_or("ws://127.0.0.1:8720");
    let expect: usize = arg(args, "--expect-snapshots")
        .unwrap_or("14")
        .parse()
        .map_err(|_| "--expect-snapshots")?;
    let max = Duration::from_secs_f64(
        arg(args, "--max-seconds")
            .unwrap_or("120")
            .parse()
            .map_err(|_| "--max-seconds")?,
    );
    let host = url
        .strip_prefix("ws://")
        .ok_or("--url must start with ws://")?;
    let stream = std::net::TcpStream::connect(host).map_err(|e| format!("{host}: {e}"))?;
    stream.set_nodelay(true).ok();
    let (mut ws, _) = tungstenite::client(url, stream).map_err(|e| format!("handshake: {e}"))?;

    let t0 = Instant::now();
    let mut frames = 0u64;
    let mut first: Option<u64> = None;
    let mut prev: Option<u64> = None;
    let mut gaps = 0u64;
    let mut first_idr = None;
    let mut bytes = 0u64;
    let mut seen: Vec<u32> = Vec::new();
    let mut sent_inputs: Vec<(u64, Instant)> = Vec::new();
    let mut acked: Vec<(u64, f64)> = Vec::new();
    let mut next_input = 1u64;
    let mut last_rx: Option<Instant> = None;
    let mut intervals = Vec::new();
    while t0.elapsed() < max && seen.len() < expect {
        let msg = ws.read().map_err(|e| format!("read: {e}"))?;
        let Message::Binary(b) = msg else { continue };
        let now = Instant::now();
        if let Some(l) = last_rx {
            intervals.push(now.duration_since(l).as_secs_f64() * 1e3);
        }
        last_rx = Some(now);
        let (meta, h264) = split_frame(&b).ok_or("malformed frame message")?;
        let v: serde_json::Value = serde_json::from_str(meta).map_err(|e| e.to_string())?;
        let n = v["frame"].as_u64().ok_or("frame")?;
        if first.is_none() {
            first = Some(n);
            first_idr = v["idr"].as_bool();
        }
        if let Some(p) = prev
            && n != p + 1
        {
            gaps += 1;
        }
        prev = Some(n);
        frames += 1;
        bytes += h264.len() as u64;
        for s in v["visible"].as_array().into_iter().flatten() {
            if let Some(s) = s.as_u64() {
                seen.push(s as u32);
            }
        }
        let input = v["input"].as_u64().unwrap_or(0);
        for (id, at) in &sent_inputs {
            if *id <= input && !acked.iter().any(|(a, _)| a == id) {
                acked.push((*id, now.duration_since(*at).as_secs_f64() * 1e3));
            }
        }
        if frames.is_multiple_of(60) {
            let text = format!(r#"{{"id":{next_input},"type":"orbit","dx":4,"dy":0}}"#);
            ws.send(Message::Text(text.into()))
                .map_err(|e| format!("send: {e}"))?;
            sent_inputs.push((next_input, Instant::now()));
            next_input += 1;
        }
    }
    let secs = t0.elapsed().as_secs_f64();
    seen.sort_unstable();
    seen.dedup();
    let mut ack_ms: Vec<f64> = acked.iter().map(|(_, ms)| *ms).collect();
    let unacked = sent_inputs.len() - acked.len();
    println!(
        "{{\"frames\":{frames},\"seconds\":{secs:.2},\"fps\":{:.2},\"first_idr\":{},\"gaps\":{gaps},\"bytes\":{bytes},\"mbps\":{:.2},\"snapshots_seen\":{:?},\"inputs_sent\":{},\"inputs_unacked\":{unacked},\"input_ack_ms_p50\":{:.2},\"input_ack_ms_max\":{:.2},\"rx_interval_ms_p50\":{:.2},\"rx_interval_ms_p99\":{:.2},\"rx_interval_ms_max\":{:.2}}}",
        frames as f64 / secs,
        first_idr.unwrap_or(false),
        bytes as f64 * 8.0 / secs / 1e6,
        seen,
        sent_inputs.len(),
        pct(&mut ack_ms.clone(), 0.5),
        ack_ms.iter_mut().fold(0.0f64, |a, b| a.max(*b)),
        pct(&mut intervals.clone(), 0.5),
        pct(&mut intervals.clone(), 0.99),
        intervals.iter().fold(0.0f64, |a, b| a.max(*b)),
    );
    let mut failures = Vec::new();
    if first_idr != Some(true) {
        failures.push("first frame is not an IDR".to_string());
    }
    if gaps > 0 {
        failures.push(format!("{gaps} gaps in frame numbers"));
    }
    if seen.len() < expect {
        failures.push(format!("saw {} of {expect} snapshots", seen.len()));
    }
    if unacked > 1 {
        // The last input may still be in flight when the probe stops.
        failures.push(format!("{unacked} inputs never acknowledged"));
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_frame_messages() {
        let mut m = 7u32.to_le_bytes().to_vec();
        m.extend(b"{\"a\":1}");
        m.extend([0, 0, 1, 9]);
        assert_eq!(split_frame(&m), Some(("{\"a\":1}", &[0u8, 0, 1, 9][..])));
        assert_eq!(split_frame(&[1, 0]), None);
        assert_eq!(split_frame(&[9, 0, 0, 0, b'x']), None);
    }
}
