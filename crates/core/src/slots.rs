//! Points in slots, changed by slot diff messages (decision 0052).
//!
//! One GPU vertex buffer, one 16-byte entry per slot id: `f32 x y z` relative to the stream origin,
//! `u8 r g b`, `u8 flags` (bit 0: shown). An add or a move overwrites its slot and shows it, a
//! remove hides it; the point pipeline draws `capacity` instances and the vertex shader sends hidden
//! slots outside the clip volume.
//!
//! Upload (decision 0052 §2), chosen per scene with [`SlotScene::with_upload`]; both write the same
//! bytes and a message is on screen whole from the next submitted frame (no half-applied frame):
//! - [`UploadMethod::Mirror`], method (a), the default: the records go into a CPU mirror of the
//!   buffer and the changed slots' lowest…highest range is written with one `write_buffer`. Every
//!   backend including WebGL2.
//! - [`UploadMethod::Scatter`], method (b): the message's pack is written as it is to a storage
//!   buffer and a compute shader scatters the records into the slots (`scatter.wgsl`). WebGPU and
//!   native; a device without compute shaders or storage buffers (WebGL2) uses the mirror instead.
//!   There is no CPU mirror; the CPU keeps one shown bit per slot (for [`SlotScene::points`]) and
//!   the box of the positions written (for [`SlotScene::bounds`]).
//!
//! When the capacity outgrows the buffer it doubles and the old contents are copied on the GPU.
//!
//! Messages are applied in `seq` order from seq 1: one that skips ahead is held until the missing
//! ones arrive, one at or before the last applied one is dropped. A scene starts later only from a
//! folded state ([`SlotScene::apply_state`]) or an explicit [`SlotScene::expect`]; a client that
//! cannot get a message recovers the same way (fetch the folded state through it, `apply_state`).

mod scatter;

use glam::DVec3;
use pointblitz_io::slots::{self, ADD_BYTES, Diff, MOVE_BYTES, SlotError};
use scatter::Scatter;
use std::collections::BTreeMap;

/// Bytes per slot on the GPU.
pub const SLOT_BYTES: usize = 16;
/// Flags bit 0: the slot is shown.
pub const FLAG_SHOWN: u8 = 1;
/// Smallest buffer, in slots.
const MIN_SLOTS: u32 = 1024;

/// How [`SlotScene`] writes messages to the GPU (decision 0052 §2; chosen by measurement, §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UploadMethod {
    /// (a) CPU mirror, one `write_buffer` of the changed range. Every backend.
    #[default]
    Mirror,
    /// (b) The pack as it is, scattered by a compute shader. WebGPU and native; falls back to
    /// `Mirror` on a device without compute shaders or storage buffers (WebGL2).
    Scatter,
}

impl UploadMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mirror => "mirror",
            Self::Scatter => "scatter",
        }
    }

    /// True when `device` can use this method (`Scatter` needs compute shaders and storage
    /// buffers).
    pub fn supported(self, device: &wgpu::Device) -> bool {
        match self {
            Self::Mirror => true,
            Self::Scatter => Scatter::supported(device),
        }
    }
}

impl std::str::FromStr for UploadMethod {
    type Err = String;

    /// `mirror` or `scatter` (also `a` / `b`, as in decision 0052).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mirror" | "a" => Ok(Self::Mirror),
            "scatter" | "b" => Ok(Self::Scatter),
            _ => Err(format!("upload method {s:?}: mirror|scatter")),
        }
    }
}

impl std::fmt::Display for UploadMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What [`SlotScene::apply`] did with a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applied {
    /// Applied, together with held messages that followed it: their seqs in order.
    Shown(Vec<u32>),
    /// A seq is missing before this one; held until it arrives.
    Held,
    /// At or before the last applied seq; dropped.
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyError {
    Message(SlotError),
    /// The device cannot hold a buffer of this many slots.
    TooManySlots {
        capacity: u32,
        max: u64,
    },
}

impl From<SlotError> for ApplyError {
    fn from(e: SlotError) -> Self {
        Self::Message(e)
    }
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Message(e) => e.fmt(f),
            Self::TooManySlots { capacity, max } => {
                write!(f, "{capacity} slots, the device holds at most {max}")
            }
        }
    }
}

impl std::error::Error for ApplyError {}

pub struct SlotScene {
    /// Stream origin (ENU metres); slot positions are relative to it.
    origin: DVec3,
    /// The method asked for.
    requested: UploadMethod,
    /// The method in use: `upload`, or `Mirror` where the device cannot scatter. Decided when the
    /// buffer is created.
    method: UploadMethod,
    /// Mirror: what the GPU buffer holds, `capacity` slots.
    mirror: Vec<u8>,
    /// Scatter: the shown bit of every slot (the mirror's flags otherwise).
    shown: Vec<u64>,
    /// Scatter: box of every position written since the scene was cleared.
    extent: Option<([f32; 3], [f32; 3])>,
    /// Scatter: kernels and the pack buffer, kept across `clear`.
    scatter: Option<Scatter>,
    /// Scatter: lower binding size and workgroup limits than the device's (tests).
    scatter_cap: Option<(u64, u32)>,
    buffer: Option<wgpu::Buffer>,
    /// Slots the GPU buffer has room for.
    allocated: u32,
    /// Slots drawn: highest id + 1 of the messages applied.
    capacity: u32,
    points: u64,
    applied: Option<u32>,
    /// The seq `apply` takes next: 1 for a new scene (the stream's first event), the one after the
    /// last applied, or what `expect` set.
    next: u32,
    held: BTreeMap<u32, Vec<u8>>,
    /// Bytes the last `apply` wrote to the GPU.
    last_upload: u64,
}

impl SlotScene {
    /// An empty scene whose slot positions are relative to `origin` (the stream's ENU origin; 0
    /// for cumulus3d's `enu` frame). Uploads with [`UploadMethod::Mirror`].
    pub fn new(origin: [f64; 3]) -> Self {
        Self {
            origin: DVec3::from(origin),
            requested: UploadMethod::Mirror,
            method: UploadMethod::Mirror,
            mirror: Vec::new(),
            shown: Vec::new(),
            extent: None,
            scatter: None,
            scatter_cap: None,
            buffer: None,
            allocated: 0,
            capacity: 0,
            points: 0,
            applied: None,
            next: 1,
            held: BTreeMap::new(),
            last_upload: 0,
        }
    }

    /// Uploads with `method` (decision 0052 §2). Takes effect from the next buffer: right away for
    /// a new scene, after [`SlotScene::clear`] (or `apply_state`) otherwise.
    pub fn with_upload(mut self, method: UploadMethod) -> Self {
        self.set_upload(method);
        self
    }

    /// Like `with_upload`, in place.
    pub fn set_upload(&mut self, method: UploadMethod) {
        self.requested = method;
        if self.buffer.is_none() {
            self.method = method;
        }
    }

    /// The method in use: the one asked for, or `Mirror` once a device without compute shaders or
    /// storage buffers created the buffer.
    pub fn upload(&self) -> UploadMethod {
        self.method
    }

    /// Scatter with storage bindings of at most `binding_bytes` and at most `groups` workgroups
    /// per dimension (below the device's limits), so tests can exercise the chunking.
    #[doc(hidden)]
    pub fn with_scatter_limits(mut self, binding_bytes: u64, groups: u32) -> Self {
        self.scatter_cap = Some((binding_bytes, groups));
        self.scatter = None;
        self
    }

    pub fn origin(&self) -> DVec3 {
        self.origin
    }

    /// Slots drawn (highest id + 1 so far).
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Slots shown.
    pub fn points(&self) -> u64 {
        self.points
    }

    /// Seq of the last applied message.
    pub fn seq(&self) -> Option<u32> {
        self.applied
    }

    /// Messages held for a missing seq.
    pub fn held(&self) -> usize {
        self.held.len()
    }

    /// GPU bytes of the slot buffer.
    pub fn gpu_bytes(&self) -> u64 {
        self.buffer.as_ref().map_or(0, wgpu::Buffer::size)
    }

    /// Bytes the last [`SlotScene::apply`] wrote to the GPU: the changed range (mirror) or the
    /// packs (scatter).
    pub fn last_upload_bytes(&self) -> u64 {
        self.last_upload
    }

    pub(crate) fn buffer(&self) -> Option<&wgpu::Buffer> {
        self.buffer.as_ref()
    }

    /// Forgets everything; the next message taken is seq 1 again (or what `expect` sets). The
    /// upload method stays.
    pub fn clear(&mut self) {
        let mut fresh = Self::new(self.origin.to_array()).with_upload(self.requested);
        fresh.scatter = self.scatter.take();
        fresh.scatter_cap = self.scatter_cap;
        *self = fresh;
    }

    /// Makes `seq` the next message taken, for a scene that joins a stream later without a folded
    /// state (the slots it does not get stay hidden). Held messages before it are dropped.
    pub fn expect(&mut self, seq: u32) {
        self.next = seq;
        self.held.retain(|s, _| *s > seq);
    }

    /// Applies one message (validated first; a bad one changes nothing) and any held messages that
    /// follow it, then writes them to the GPU (one `write_buffer` of the changed range, or one
    /// scatter per message). Only the expected seq is applied — seq 1 for a new scene; a later one
    /// is held until the ones before it arrive, an earlier one is dropped. A folded state starts a
    /// scene through [`SlotScene::apply_state`].
    pub fn apply(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        msg: &[u8],
    ) -> Result<Applied, ApplyError> {
        let header = slots::validate(msg)?;
        // Checked on arrival, so a message that can never fit is not held in front of good ones.
        let max = device.limits().max_buffer_size / SLOT_BYTES as u64;
        if u64::from(header.capacity) > max {
            return Err(ApplyError::TooManySlots {
                capacity: header.capacity,
                max,
            });
        }
        if header.seq < self.next {
            return Ok(Applied::Stale);
        }
        if header.seq > self.next {
            self.held.insert(header.seq, msg.to_vec());
            return Ok(Applied::Held);
        }
        Ok(self.apply_in_order(device, queue, msg, header.seq, max))
    }

    /// Clears the scene and applies a folded state (`slots::Fold`, `/slots/state?seq=N`) whatever
    /// its seq; held messages after it stay and follow it. This is also how a client recovers from
    /// a message it could not get.
    pub fn apply_state(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        msg: &[u8],
    ) -> Result<Applied, ApplyError> {
        let header = slots::validate(msg)?;
        let max = device.limits().max_buffer_size / SLOT_BYTES as u64;
        if u64::from(header.capacity) > max {
            return Err(ApplyError::TooManySlots {
                capacity: header.capacity,
                max,
            });
        }
        let held = std::mem::take(&mut self.held);
        self.clear();
        self.held = held;
        self.expect(header.seq);
        Ok(self.apply_in_order(device, queue, msg, header.seq, max))
    }

    /// Applies `msg` (seq `self.next`) and the held messages that follow it.
    fn apply_in_order(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        msg: &[u8],
        seq: u32,
        max: u64,
    ) -> Applied {
        let mut capacity = self
            .capacity
            .max(slots::decode_header(msg).map_or(0, |h| h.capacity));
        let mut next = seq.checked_add(1);
        while let Some(m) = next.and_then(|n| self.held.get(&n)) {
            capacity = capacity.max(slots::decode_header(m).map_or(0, |h| h.capacity));
            next = next.and_then(|n| n.checked_add(1));
        }
        if self.buffer.is_none() {
            self.method = if self.requested.supported(device) {
                self.requested
            } else {
                UploadMethod::Mirror
            };
        }
        self.grow(capacity);
        // The grow copy is submitted first; the mirror's write runs at the next submit, each scatter
        // is its own submit, so both see the grown buffer.
        self.reserve(device, queue, max);
        self.last_upload = 0;
        let mut dirty: Option<(u32, u32)> = None;
        self.take(device, queue, msg, &mut dirty);
        let mut shown = vec![seq];
        while let Some(m) = self.held.remove(&self.next) {
            self.take(device, queue, &m, &mut dirty);
            shown.push(self.applied.expect("set by take"));
        }
        if let (Some((lo, hi)), Some(buffer)) = (dirty, &self.buffer) {
            let range = lo as usize * SLOT_BYTES..(hi as usize + 1) * SLOT_BYTES;
            self.last_upload = range.len() as u64;
            queue.write_buffer(buffer, range.start as u64, &self.mirror[range]);
        }
        Applied::Shown(shown)
    }

    /// Room on the CPU side for `capacity` slots.
    fn grow(&mut self, capacity: u32) {
        if capacity > self.capacity {
            self.capacity = capacity;
            match self.method {
                UploadMethod::Mirror => {
                    let len = capacity as usize * SLOT_BYTES;
                    // Amortised doubling on purpose: it can hold up to twice the bytes (73.5 MB for
                    // theater30's 46 MB), but an exact allocation reallocates on almost every event
                    // (ids creep up by a few thousand each) and cost about 15 ms per replace event,
                    // an eighth of headroom still about 6 ms on the events where the capacity
                    // jumps (PR #94). Memory is recorded, not traded for latency (INTENT principle
                    // 2, SPEC §7.1).
                    self.mirror.resize(len, 0);
                }
                UploadMethod::Scatter => self.shown.resize(capacity.div_ceil(64) as usize, 0),
            }
        }
    }

    /// Applies a validated message: to the mirror (widening `dirty` to the slots it changed), or to
    /// the shown bits and the GPU through the scatter kernels.
    fn take(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        msg: &[u8],
        dirty: &mut Option<(u32, u32)>,
    ) {
        let d = Diff::parse(msg).expect("validated before");
        match self.method {
            UploadMethod::Mirror => self.write_mirror(&d, dirty),
            UploadMethod::Scatter => {
                self.mark(&d);
                // No buffer: capacity 0, so the message has no records.
                let Some(buffer) = self.buffer.as_ref() else {
                    self.applied = Some(d.header.seq);
                    self.next = d.header.seq + 1;
                    return;
                };
                let cap = self.scatter_cap;
                let scatter = self
                    .scatter
                    .get_or_insert_with(|| Scatter::new(device, cap));
                self.last_upload +=
                    scatter.apply(device, queue, buffer, self.capacity, &d.header, msg);
            }
        }
        self.applied = Some(d.header.seq);
        // `validate` rejects seq u32::MAX, so there is always a next one.
        self.next = d.header.seq + 1;
    }

    fn write_mirror(&mut self, d: &Diff<'_>, dirty: &mut Option<(u32, u32)>) {
        let mut touch = |id: u32| {
            *dirty = Some(dirty.map_or((id, id), |(lo, hi)| (lo.min(id), hi.max(id))));
        };
        let mut points = self.points;
        let adds = d.adds.as_chunks::<ADD_BYTES>().0;
        let moves = d.moves.as_chunks::<MOVE_BYTES>().0;
        for rec in adds.iter().chain(moves) {
            let id = u32::from_le_bytes(rec[..4].try_into().unwrap());
            touch(id);
            let s = &mut self.mirror[id as usize * SLOT_BYTES..][..SLOT_BYTES];
            // f32 x y z and u8 r g b as they are in the record; the record's zone byte is not used.
            s[..15].copy_from_slice(&rec[4..19]);
            points += u64::from(s[15] & FLAG_SHOWN == 0);
            s[15] |= FLAG_SHOWN;
        }
        for id in d.remove_ids() {
            touch(id);
            let flags = &mut self.mirror[id as usize * SLOT_BYTES + 15];
            points -= u64::from(*flags & FLAG_SHOWN != 0);
            *flags &= !FLAG_SHOWN;
        }
        self.points = points;
    }

    /// Scatter: the CPU's share of a message, the shown bits (the point count) and the extent.
    fn mark(&mut self, d: &Diff<'_>) {
        let mut points = self.points;
        let (mut lo, mut hi) = self
            .extent
            .unwrap_or(([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]));
        let adds = d.adds.as_chunks::<ADD_BYTES>().0;
        let moves = d.moves.as_chunks::<MOVE_BYTES>().0;
        for rec in adds.iter().chain(moves) {
            let id = u32::from_le_bytes(rec[..4].try_into().unwrap()) as usize;
            let (word, bit) = (&mut self.shown[id / 64], 1u64 << (id % 64));
            points += u64::from(*word & bit == 0);
            *word |= bit;
            for a in 0..3 {
                let v = f32::from_le_bytes(rec[4 + 4 * a..8 + 4 * a].try_into().unwrap());
                lo[a] = lo[a].min(v);
                hi[a] = hi[a].max(v);
            }
        }
        for id in d.remove_ids() {
            let id = id as usize;
            let (word, bit) = (&mut self.shown[id / 64], 1u64 << (id % 64));
            points -= u64::from(*word & bit != 0);
            *word &= !bit;
        }
        if !adds.is_empty() || !moves.is_empty() {
            self.extent = Some((lo, hi));
        }
        self.points = points;
    }

    /// Makes room for `capacity` slots on the GPU (doubling, old contents copied on the GPU).
    fn reserve(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, max: u64) {
        if self.capacity <= self.allocated {
            return;
        }
        let want = u64::from(
            self.capacity
                .max(self.allocated.saturating_mul(2))
                .max(MIN_SLOTS),
        )
        .min(max)
        .max(u64::from(self.capacity)) as u32;
        let mut usage = wgpu::BufferUsages::VERTEX
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC;
        if self.method == UploadMethod::Scatter {
            usage |= wgpu::BufferUsages::STORAGE;
        }
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("slots"),
            size: u64::from(want) * SLOT_BYTES as u64,
            usage,
            mapped_at_creation: false,
        });
        if let Some(old) = &self.buffer {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("slots grow"),
            });
            enc.copy_buffer_to_buffer(old, 0, &buffer, 0, old.size());
            queue.submit([enc.finish()]);
        }
        self.buffer = Some(buffer);
        self.allocated = want;
    }

    /// The first `capacity` slots as the GPU holds them (16 B each), read back and waited for.
    /// For tests and tools comparing upload methods; not on wasm32 (no blocking wait there).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn read_slots(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<u8> {
        let len = u64::from(self.capacity) * SLOT_BYTES as u64;
        let Some(buffer) = self.buffer.as_ref().filter(|_| len > 0) else {
            return Vec::new();
        };
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("slots readback"),
            size: len,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(buffer, 0, &readback, 0, len);
        let index = queue.submit([enc.finish()]);
        readback.map_async(wgpu::MapMode::Read, .., |r| r.expect("map slots"));
        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(index),
                timeout: None,
            })
            .expect("GPU wait");
        let out = readback.get_mapped_range(..).expect("mapped").to_vec();
        readback.unmap();
        out
    }

    /// Bounds of the shown slots in absolute ENU metres, or None when nothing is shown. With
    /// scatter (no CPU copy of the positions) it is the box of every position written since the
    /// scene was cleared, which holds the shown slots and is larger when some were removed or
    /// moved away.
    pub fn bounds(&self) -> Option<([f64; 3], [f64; 3])> {
        if self.points == 0 {
            return None;
        }
        let (min, max) = match self.method {
            UploadMethod::Scatter => {
                let (lo, hi) = self.extent?;
                (lo.map(f64::from), hi.map(f64::from))
            }
            UploadMethod::Mirror => {
                let mut min = [f64::INFINITY; 3];
                let mut max = [f64::NEG_INFINITY; 3];
                for s in self.mirror.as_chunks::<SLOT_BYTES>().0 {
                    if s[15] & FLAG_SHOWN == 0 {
                        continue;
                    }
                    for a in 0..3 {
                        let v =
                            f64::from(f32::from_le_bytes(s[4 * a..4 * a + 4].try_into().unwrap()));
                        min[a] = min[a].min(v);
                        max[a] = max[a].max(v);
                    }
                }
                (min, max)
            }
        };
        let o = self.origin.to_array();
        Some((
            [0, 1, 2].map(|a| o[a] + min[a]),
            [0, 1, 2].map(|a| o[a] + max[a]),
        ))
    }
}
