//! Snapshot → chunks, shared by the replay server and a video server reading local data
//! (decisions 0026, 0050).
//!
//! - [`plan`]: full or delta delivery for what a client holds (`have = (generation, points)`).
//! - [`Conversion`]: reads the records a delivery sends and encodes them in parallel in the
//!   background; readers take chunks in order as soon as each is ready, so the first chunk can leave
//!   before the last is encoded.

use crate::chunk::{FLAG_LAST_IN_GENERATION, chunk_count, chunk_records, encode_records};
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

/// Largest chunk (decision 0022).
pub const MAX_POINTS: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    Full { generation: u32 },
    Delta { generation: u32, skip: usize },
}

/// One snapshot as [`plan`] sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub seq: u32,
    pub preview: bool,
    pub points: usize,
}

/// Parses `have=<generation>.<points>`.
pub fn parse_have(v: &str) -> Option<(u32, usize)> {
    let (g, n) = v.split_once('.')?;
    Some((g.parse().ok()?, n.parse().ok()?))
}

/// Chooses the delivery for `steps[idx]`. `appends(k)` says whether snapshot `k` (k ≥ 1) holds every
/// record of snapshot `k − 1` unchanged at its start. A delta needs a preview, a client holding
/// exactly the previous snapshot, and a chain of appending previews back to the client's
/// generation; anything else is a full delivery named by the snapshot.
pub fn plan(
    steps: &[Step],
    idx: usize,
    have: Option<(u32, usize)>,
    mut appends: impl FnMut(usize) -> bool,
) -> Plan {
    let e = steps[idx];
    let full = Plan::Full { generation: e.seq };
    let Some((generation, points)) = have else {
        return full;
    };
    if !e.preview || idx == 0 || steps[idx - 1].points != points {
        return full;
    }
    let Some(start) = steps.iter().position(|x| x.seq == generation) else {
        return full;
    };
    if start >= idx {
        return full;
    }
    if (start + 1..=idx).all(|k| steps[k].preview && appends(k)) {
        Plan::Delta {
            generation,
            skip: points,
        }
    } else {
        full
    }
}

/// What a client that followed every announcement holds before `steps[idx]`, so a delta can be
/// prepared before anyone asks: the previous snapshot, in the generation its chain of appending
/// previews started. `None` for the first snapshot.
pub fn expected_have(
    steps: &[Step],
    idx: usize,
    mut appends: impl FnMut(usize) -> bool,
) -> Option<(u32, usize)> {
    if idx == 0 {
        return None;
    }
    let mut start = idx - 1;
    while start > 0 && steps[start].preview && appends(start) {
        start -= 1;
    }
    Some((steps[start].seq, steps[idx - 1].points))
}

#[derive(Default)]
struct State {
    /// Encoded chunks in order; a taken chunk leaves an empty vector behind.
    chunks: Vec<Option<Vec<u8>>>,
    /// Chunks in the delivery, once the header was read.
    total: Option<usize>,
    error: Option<String>,
    read_ms: f64,
    done_ms: Option<f64>,
}

/// One delivery being converted in the background (see the module docs).
pub struct Conversion {
    state: Mutex<State>,
    ready: Condvar,
    started: Instant,
    pub skip: usize,
    pub generation: u32,
}

impl Conversion {
    /// Starts converting the records after the first `skip` points of the PLY at `path`, as chunks
    /// of `generation`. Encoding uses up to one thread per core.
    pub fn start(path: PathBuf, skip: usize, generation: u32) -> Arc<Self> {
        let c = Arc::new(Self {
            state: Mutex::new(State::default()),
            ready: Condvar::new(),
            started: Instant::now(),
            skip,
            generation,
        });
        let work = Arc::clone(&c);
        std::thread::spawn(move || {
            if let Err(e) = work.run(&path) {
                let mut s = work.lock();
                s.error = Some(format!("{}: {e}", path.display()));
                work.ready.notify_all();
            }
        });
        c
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn run(&self, path: &std::path::Path) -> std::io::Result<()> {
        let invalid = |e: crate::PlyError| std::io::Error::new(std::io::ErrorKind::InvalidData, e);
        let mut file = std::fs::File::open(path)?;
        let mut head = Vec::new();
        (&mut file).take(64 * 1024).read_to_end(&mut head)?;
        let h = crate::parse_header(&head).map_err(invalid)?;
        let skip = self.skip.min(h.vertex_count);
        let mut records = vec![0u8; (h.vertex_count - skip) * h.stride];
        file.seek(SeekFrom::Start((h.header_len + skip * h.stride) as u64))?;
        file.read_exact(&mut records)?;
        let chunks = chunk_count(h.vertex_count - skip, MAX_POINTS);
        {
            let mut s = self.lock();
            s.total = Some(chunks);
            s.chunks = vec![None; chunks];
            s.read_ms = self.started.elapsed().as_secs_f64() * 1e3;
        }
        self.ready.notify_all();
        let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
        let next = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..workers.min(chunks) {
                scope.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if i >= chunks {
                            break;
                        }
                        let flags = if i + 1 == chunks {
                            FLAG_LAST_IN_GENERATION
                        } else {
                            0
                        };
                        let part = chunk_records(&h, &records, MAX_POINTS, i);
                        let c = encode_records(&h, part, self.generation, i as u32, flags);
                        self.lock().chunks[i] = Some(c);
                        self.ready.notify_all();
                    }
                });
            }
        });
        self.lock().done_ms = Some(self.started.elapsed().as_secs_f64() * 1e3);
        self.ready.notify_all();
        Ok(())
    }

    /// Number of chunks, waiting for the header if needed.
    pub fn len(&self) -> Result<usize, String> {
        let mut s = self.lock();
        loop {
            if let Some(e) = &s.error {
                return Err(e.clone());
            }
            if let Some(n) = s.total {
                return Ok(n);
            }
            s = self.ready.wait(s).unwrap_or_else(|e| e.into_inner());
        }
    }

    /// Never empty once converted (a delivery has at least one chunk).
    pub fn is_empty(&self) -> bool {
        self.len().is_ok_and(|n| n == 0)
    }

    /// Chunk `i`, waiting until it is encoded. The bytes stay for other readers.
    pub fn chunk(&self, i: usize) -> Result<Vec<u8>, String> {
        self.wait(i, |slot| slot.clone())
    }

    /// Chunk `i`, waiting until it is encoded, moved out — for a single reader that keeps nothing.
    pub fn take(&self, i: usize) -> Result<Vec<u8>, String> {
        self.wait(i, |slot| slot.take())
    }

    fn wait(
        &self,
        i: usize,
        get: impl Fn(&mut Option<Vec<u8>>) -> Option<Vec<u8>>,
    ) -> Result<Vec<u8>, String> {
        let mut s = self.lock();
        loop {
            if let Some(e) = &s.error {
                return Err(e.clone());
            }
            if let Some(slot) = s.chunks.get_mut(i)
                && slot.is_some()
            {
                return Ok(get(slot).expect("checked"));
            }
            if s.total.is_some_and(|n| i >= n) {
                return Err(format!("no chunk {i}"));
            }
            if s.done_ms.is_some() {
                return Err(format!("chunk {i} was taken"));
            }
            s = self.ready.wait(s).unwrap_or_else(|e| e.into_inner());
        }
    }

    /// (file read ms, all chunks encoded ms) since the start; the second is `None` while running.
    pub fn times(&self) -> (f64, Option<f64>) {
        let s = self.lock();
        (s.read_ms, s.done_ms)
    }

    /// Bytes held (encoded chunks not taken yet).
    pub fn held_bytes(&self) -> usize {
        self.lock().chunks.iter().flatten().map(Vec::len).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(seq: u32, preview: bool, points: usize) -> Step {
        Step {
            seq,
            preview,
            points,
        }
    }

    fn flight() -> Vec<Step> {
        vec![
            s(1, true, 100),
            s(2, false, 200),
            s(3, true, 230),
            s(4, true, 260),
            s(5, false, 400),
        ]
    }

    #[test]
    fn plan_matches_the_replay_rules() {
        let f = flight();
        assert_eq!(
            plan(&f, 3, Some((2, 230)), |_| true),
            Plan::Delta {
                generation: 2,
                skip: 230
            }
        );
        assert_eq!(
            plan(&f, 4, Some((2, 260)), |_| true),
            Plan::Full { generation: 5 }
        );
        assert_eq!(plan(&f, 0, None, |_| true), Plan::Full { generation: 1 });
    }

    #[test]
    fn expected_have_is_what_a_follower_holds() {
        let f = flight();
        assert_eq!(expected_have(&f, 0, |_| true), None);
        // Before 3: holds 2 (refined) as generation 2.
        assert_eq!(expected_have(&f, 2, |_| true), Some((2, 200)));
        // Before 4: 3 appended to 2, so still generation 2.
        assert_eq!(expected_have(&f, 3, |_| true), Some((2, 230)));
        // If 3 did not append, 3 started its own generation.
        assert_eq!(expected_have(&f, 3, |k| k != 2), Some((3, 230)));
        // And the plan for 4 with that is a delta exactly when it should be.
        let have = expected_have(&f, 3, |_| true);
        assert!(matches!(plan(&f, 3, have, |_| true), Plan::Delta { .. }));
    }

    #[test]
    fn conversion_matches_encoding_in_one_go() {
        let mut bytes = b"ply\nformat binary_little_endian 1.0\nelement vertex 5\n\
            property float x\nproperty float y\nproperty float z\n\
            property uchar red\nproperty uchar green\nproperty uchar blue\nend_header\n"
            .to_vec();
        for i in 0..5u8 {
            for v in [f32::from(i), 1.0, -2.0] {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            bytes.extend_from_slice(&[i, 2 * i, 3 * i]);
        }
        let path = std::env::temp_dir().join(format!("pb-convert-{}.ply", std::process::id()));
        std::fs::write(&path, &bytes).unwrap();
        let h = crate::parse_header(&bytes).unwrap();
        for skip in [0, 2] {
            let c = Conversion::start(path.clone(), skip, 7);
            assert_eq!(c.len(), Ok(1));
            let want = encode_records(
                &h,
                &bytes[h.header_len + skip * h.stride..],
                7,
                0,
                FLAG_LAST_IN_GENERATION,
            );
            assert_eq!(c.chunk(0).unwrap(), want);
            assert_eq!(c.take(0).unwrap(), want);
            assert!(c.chunk(1).is_err());
        }
        std::fs::remove_file(&path).ok();
    }
}
