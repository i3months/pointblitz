//! A cumulus3d v4 `live/stream` directory (decision 0052): `manifest.json` plus `packs/e_NNNN.bin`.
//!
//! The manifest is rewritten atomically after each event's pack, so every event it lists has its
//! pack on disk. [`StreamDir::refresh`] re-reads it; events come out in `seq` order with their
//! message ([`super::encode`] of the pack as it is). The per-event capacity is not in the manifest
//! (only the latest one is): it is the highest add id + 1 so far, so earlier packs are scanned once.

use super::{SlotError, SlotHeader, add_capacity, encode};
use std::path::{Path, PathBuf};

/// One event of the manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamEvent {
    pub seq: u32,
    /// Seconds since the session start.
    pub t: f64,
    /// The zone that triggered the event.
    pub zone: u32,
    /// `refined`, `replace` or `final`.
    pub kind: String,
    /// Points shown after the event.
    pub points: u64,
    /// Pack path relative to the directory; none when nothing changed.
    pub pack: Option<String>,
    pub n_add: u32,
    pub n_move: u32,
    pub n_remove: u32,
    /// Pack bytes.
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    pub version: u32,
    /// `enu` (metres, x east, y north, z up) or `display`.
    pub frame: String,
    /// Geodetic origin `[lat, lon, alt]` of the ENU frame, when known.
    pub origin_gps: Option<[f64; 3]>,
    /// Highest id + 1 used so far.
    pub capacity: u32,
    /// In `seq` order.
    pub events: Vec<StreamEvent>,
}

/// The manifest format this module reads.
pub const MANIFEST_VERSION: u32 = 4;

/// Parses `manifest.json` (version 4).
pub fn parse_manifest(text: &str) -> Result<Manifest, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let u32_of = |v: &serde_json::Value, k: &str| -> Result<u32, String> {
        v[k].as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(format!("manifest: {k}"))
    };
    let version = u32_of(&v, "version")?;
    if version != MANIFEST_VERSION {
        return Err(format!(
            "manifest version {version}, expected {MANIFEST_VERSION}"
        ));
    }
    let origin_gps = match v["origin_gps"].as_array() {
        Some(a) if a.len() == 3 => {
            let f = |i: usize| a[i].as_f64().ok_or("manifest: origin_gps");
            Some([f(0)?, f(1)?, f(2)?])
        }
        _ => None,
    };
    let mut events = Vec::new();
    for e in v["events"].as_array().ok_or("manifest: events")? {
        events.push(StreamEvent {
            seq: u32_of(e, "seq")?,
            t: e["t"].as_f64().ok_or("manifest: t")?,
            zone: u32_of(e, "zone")?,
            kind: e["kind"].as_str().ok_or("manifest: kind")?.to_string(),
            points: e["points"].as_u64().ok_or("manifest: points")?,
            pack: e["pack"].as_str().map(str::to_string),
            n_add: u32_of(e, "n_add")?,
            n_move: u32_of(e, "n_move")?,
            n_remove: u32_of(e, "n_remove")?,
            bytes: e["bytes"].as_u64().ok_or("manifest: bytes")?,
        });
    }
    events.sort_by_key(|e| e.seq);
    Ok(Manifest {
        version,
        frame: v["frame"].as_str().unwrap_or("enu").to_string(),
        origin_gps,
        capacity: u32_of(&v, "capacity")?,
        events,
    })
}

fn invalid(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
}

/// Reader of one stream directory.
#[derive(Debug)]
pub struct StreamDir {
    dir: PathBuf,
    manifest: Manifest,
    /// Capacity after `manifest.events[i]`, for the events scanned so far.
    caps: Vec<u32>,
}

impl StreamDir {
    /// Reads `<dir>/manifest.json`.
    pub fn open(dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let dir = dir.into();
        let manifest = read_manifest(&dir)?;
        Ok(Self {
            dir,
            manifest,
            caps: Vec::new(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.dir
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn events(&self) -> &[StreamEvent] {
        &self.manifest.events
    }

    /// Re-reads the manifest (a live session appends events) and returns the events that are new.
    pub fn refresh(&mut self) -> std::io::Result<Vec<StreamEvent>> {
        let m = read_manifest(&self.dir)?;
        let last = self.manifest.events.last().map_or(0, |e| e.seq);
        let new = m.events.iter().filter(|e| e.seq > last).cloned().collect();
        // Earlier events never change (packs are never overwritten), so the scanned capacities stay.
        self.caps.truncate(m.events.len());
        self.manifest = m;
        Ok(new)
    }

    /// The pack of event `idx`, length-checked against the manifest's counts.
    fn pack(&self, idx: usize) -> std::io::Result<Vec<u8>> {
        let e = &self.manifest.events[idx];
        let pack = match &e.pack {
            Some(p) => std::fs::read(self.dir.join(p))?,
            None => Vec::new(),
        };
        let want = super::pack_len(e.n_add, e.n_move, e.n_remove);
        if pack.len() != want || pack.len() as u64 != e.bytes {
            return Err(invalid(format!(
                "event {}: pack is {} B, counts give {want} B, manifest says {} B",
                e.seq,
                pack.len(),
                e.bytes
            )));
        }
        Ok(pack)
    }

    /// Capacity before event `idx`: scans the add sections of earlier packs not scanned yet.
    fn capacity_before(&mut self, idx: usize) -> std::io::Result<u32> {
        while self.caps.len() < idx {
            let k = self.caps.len();
            let e = &self.manifest.events[k];
            let adds = match &e.pack {
                Some(p) => {
                    use std::io::Read;
                    let mut buf = vec![0u8; e.n_add as usize * super::ADD_BYTES];
                    std::fs::File::open(self.dir.join(p))?.read_exact(&mut buf)?;
                    buf
                }
                None => Vec::new(),
            };
            let before = self.caps.last().copied().unwrap_or(0);
            self.caps.push(before.max(add_capacity(&adds, e.n_add)));
        }
        Ok(if idx == 0 { 0 } else { self.caps[idx - 1] })
    }

    /// The message of event `seq` (header + its pack), validated.
    pub fn message(&mut self, seq: u32) -> std::io::Result<Vec<u8>> {
        let idx = self
            .manifest
            .events
            .iter()
            .position(|e| e.seq == seq)
            .ok_or_else(|| std::io::Error::other(format!("event {seq} not in the manifest")))?;
        let before = self.capacity_before(idx)?;
        let pack = self.pack(idx)?;
        let e = &self.manifest.events[idx];
        let capacity = before.max(add_capacity(&pack, e.n_add));
        if self.caps.len() == idx {
            self.caps.push(capacity);
        }
        let header = SlotHeader {
            seq,
            n_add: e.n_add,
            n_move: e.n_move,
            n_remove: e.n_remove,
            capacity,
        };
        let msg = encode(&header, &pack).map_err(invalid)?;
        super::validate(&msg).map_err(|e: SlotError| invalid(format!("event {seq}: {e}")))?;
        Ok(msg)
    }

    /// Every event's message, in `seq` order.
    pub fn messages(&mut self) -> std::io::Result<Vec<(StreamEvent, Vec<u8>)>> {
        let events = self.manifest.events.clone();
        events
            .into_iter()
            .map(|e| self.message(e.seq).map(|m| (e, m)))
            .collect()
    }
}

fn read_manifest(dir: &Path) -> std::io::Result<Manifest> {
    let path = dir.join("manifest.json");
    let text = std::fs::read_to_string(&path)?;
    parse_manifest(&text).map_err(|e| invalid(format!("{}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slots::{Diff, Fold, HEADER_LEN};

    fn event_json(seq: u32, kind: &str, points: u64, pack: &[u8], counts: [u32; 3]) -> String {
        let pack_field = if pack.is_empty() {
            "null".to_string()
        } else {
            format!("\"packs/e_{seq:04}.bin\"")
        };
        format!(
            r#"{{"seq":{seq},"t":{}.5,"zone":0,"kind":"{kind}","points":{points},"pack":{pack_field},"n_add":{},"n_move":{},"n_remove":{},"bytes":{}}}"#,
            seq * 10,
            counts[0],
            counts[1],
            counts[2],
            pack.len()
        )
    }

    /// A small stream in the v4 layout, written the way cumulus3d writes it.
    fn write_stream(dir: &Path) {
        use crate::slots::tests::msg;
        std::fs::create_dir_all(dir.join("packs")).unwrap();
        let events = [
            (
                "refined",
                3,
                msg(
                    1,
                    3,
                    &[(0, 1.0, [1; 3]), (1, 2.0, [2; 3]), (2, 3.0, [3; 3])],
                    &[],
                    &[],
                ),
                [3, 0, 0],
            ),
            (
                "replace",
                3,
                msg(2, 4, &[(3, 4.0, [4; 3])], &[(1, 5.0, [5; 3])], &[0]),
                [1, 1, 1],
            ),
            (
                "refined",
                4,
                msg(3, 4, &[(0, 6.0, [6; 3])], &[], &[]),
                [1, 0, 0],
            ),
            ("final", 4, msg(4, 4, &[], &[], &[]), [0, 0, 0]),
        ];
        let mut items = Vec::new();
        for (k, (kind, points, m, counts)) in events.iter().enumerate() {
            let seq = k as u32 + 1;
            let pack = &m[HEADER_LEN..];
            if !pack.is_empty() {
                std::fs::write(dir.join(format!("packs/e_{seq:04}.bin")), pack).unwrap();
            }
            items.push(event_json(seq, kind, *points, pack, *counts));
        }
        let manifest = format!(
            r#"{{"version":4,"frame":"enu","origin_gps":[36.5,127.25,29.9],"capacity":4,"events":[{}]}}"#,
            items.join(",")
        );
        std::fs::write(dir.join("manifest.json"), manifest).unwrap();
    }

    #[test]
    fn reads_a_stream_directory_in_seq_order() {
        let dir = std::env::temp_dir().join(format!("pb-slots-dir-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        write_stream(&dir);
        let mut s = StreamDir::open(&dir).unwrap();
        let m = s.manifest().clone();
        assert_eq!((m.version, m.frame.as_str(), m.capacity), (4, "enu", 4));
        assert_eq!(m.origin_gps, Some([36.5, 127.25, 29.9]));
        assert_eq!(m.events.len(), 4);
        // Asking for a later event first still gives the right capacity (earlier adds scanned).
        let third = s.message(3).unwrap();
        assert_eq!(crate::slots::validate(&third).unwrap().capacity, 4);
        let all = s.messages().unwrap();
        let caps: Vec<u32> = all
            .iter()
            .map(|(_, m)| crate::slots::validate(m).unwrap().capacity)
            .collect();
        assert_eq!(caps, vec![3, 4, 4, 4]);
        assert_eq!(all[3].1.len(), HEADER_LEN);
        // points = previous + add − remove for every event.
        let mut fold = Fold::new();
        let mut prev = 0i64;
        for (e, msg) in &all {
            let h = Diff::parse(msg).unwrap().header;
            prev += i64::from(h.n_add) - i64::from(h.n_remove);
            assert_eq!(prev, e.points as i64, "event {}", e.seq);
            fold.apply(msg).unwrap();
        }
        assert_eq!(fold.points(), 4);
        // A pack that does not match the manifest is refused.
        std::fs::write(dir.join("packs/e_0002.bin"), [0u8; 3]).unwrap();
        assert!(s.message(2).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn refresh_returns_only_new_events() {
        let dir = std::env::temp_dir().join(format!("pb-slots-refresh-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let one = format!(
            r#"{{"version":4,"frame":"enu","origin_gps":null,"capacity":0,"events":[{}]}}"#,
            event_json(1, "final", 0, &[], [0, 0, 0])
        );
        std::fs::write(dir.join("manifest.json"), one).unwrap();
        let mut s = StreamDir::open(&dir).unwrap();
        assert_eq!(s.manifest().origin_gps, None);
        assert!(s.refresh().unwrap().is_empty());
        write_stream(&dir);
        let new: Vec<u32> = s.refresh().unwrap().iter().map(|e| e.seq).collect();
        assert_eq!(new, vec![2, 3, 4]);
        assert!(parse_manifest(r#"{"version":3,"capacity":0,"events":[]}"#).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
