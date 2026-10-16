//! Points in slots, changed by slot diff messages (decision 0052).
//!
//! One GPU vertex buffer, one 16-byte entry per slot id: `f32 x y z` relative to the stream origin,
//! `u8 r g b`, `u8 flags` (bit 0: shown). An add or a move overwrites its slot and shows it, a
//! remove hides it; the point pipeline draws `capacity` instances and the vertex shader sends hidden
//! slots outside the clip volume.
//!
//! Upload (decision 0052 §2, method (a)): the records go into a CPU mirror of the buffer and the
//! changed slots' lowest…highest range is written with one `write_buffer`, so a message is on
//! screen whole from the next submitted frame (no half-applied frame), on every backend including
//! WebGL2. When the capacity outgrows the buffer it doubles and the old contents are copied on the
//! GPU.
//!
//! Messages are applied in `seq` order: one that skips ahead is held until the missing ones arrive,
//! one at or before the last applied one is dropped.

use glam::DVec3;
use pointblitz_io::slots::{self, ADD_BYTES, Diff, MOVE_BYTES, SlotError};
use std::collections::BTreeMap;

/// Bytes per slot on the GPU.
pub const SLOT_BYTES: usize = 16;
/// Flags bit 0: the slot is shown.
pub const FLAG_SHOWN: u8 = 1;
/// Smallest buffer, in slots.
const MIN_SLOTS: u32 = 1024;

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
    /// What the GPU buffer holds, `capacity` slots.
    mirror: Vec<u8>,
    buffer: Option<wgpu::Buffer>,
    /// Slots the GPU buffer has room for.
    allocated: u32,
    /// Slots drawn: highest id + 1 of the messages applied.
    capacity: u32,
    points: u64,
    applied: Option<u32>,
    held: BTreeMap<u32, Vec<u8>>,
    /// Bytes the last `apply` wrote to the GPU.
    last_upload: u64,
}

impl SlotScene {
    /// An empty scene whose slot positions are relative to `origin` (the stream's ENU origin; 0
    /// for cumulus3d's `enu` frame).
    pub fn new(origin: [f64; 3]) -> Self {
        Self {
            origin: DVec3::from(origin),
            mirror: Vec::new(),
            buffer: None,
            allocated: 0,
            capacity: 0,
            points: 0,
            applied: None,
            held: BTreeMap::new(),
            last_upload: 0,
        }
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

    /// Bytes the last [`SlotScene::apply`] wrote to the GPU (the changed range).
    pub fn last_upload_bytes(&self) -> u64 {
        self.last_upload
    }

    pub(crate) fn buffer(&self) -> Option<&wgpu::Buffer> {
        self.buffer.as_ref()
    }

    /// Forgets everything (e.g. before applying a folded state); the next message is applied
    /// whatever its seq.
    pub fn clear(&mut self) {
        *self = Self::new(self.origin.to_array());
    }

    /// Applies one message (validated first; a bad one changes nothing) and any held messages that
    /// follow it, then writes the changed slots to the GPU in one `write_buffer`. The first message
    /// after `new` or `clear` is applied whatever its seq — a folded state, or the first event.
    pub fn apply(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        msg: &[u8],
    ) -> Result<Applied, ApplyError> {
        let header = slots::validate(msg)?;
        match self.applied {
            Some(last) if header.seq <= last => return Ok(Applied::Stale),
            Some(last) if header.seq > last + 1 => {
                self.held.insert(header.seq, msg.to_vec());
                return Ok(Applied::Held);
            }
            _ => {}
        }
        let max = device.limits().max_buffer_size / SLOT_BYTES as u64;
        let mut capacity = self.capacity.max(header.capacity);
        let mut next = header.seq + 1;
        while let Some(m) = self.held.get(&next) {
            capacity = capacity.max(slots::decode_header(m)?.capacity);
            next += 1;
        }
        if u64::from(capacity) > max {
            return Err(ApplyError::TooManySlots { capacity, max });
        }

        let mut dirty: Option<(u32, u32)> = None;
        self.grow_mirror(capacity);
        self.write(msg, &mut dirty);
        let mut shown = vec![header.seq];
        while let Some(m) = self.held.remove(&(self.applied.unwrap_or(0) + 1)) {
            self.write(&m, &mut dirty);
            shown.push(self.applied.expect("set by write"));
        }
        self.upload(device, queue, max, dirty);
        Ok(Applied::Shown(shown))
    }

    fn grow_mirror(&mut self, capacity: u32) {
        if capacity > self.capacity {
            self.capacity = capacity;
            self.mirror.resize(capacity as usize * SLOT_BYTES, 0);
        }
    }

    /// Applies a validated message to the mirror and widens `dirty` to the slots it changed.
    fn write(&mut self, msg: &[u8], dirty: &mut Option<(u32, u32)>) {
        let d = Diff::parse(msg).expect("validated before");
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
        self.applied = Some(d.header.seq);
    }

    /// Makes room for `capacity` slots on the GPU (doubling, old contents copied on the GPU), then
    /// writes the dirty range of the mirror.
    fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        max: u64,
        dirty: Option<(u32, u32)>,
    ) {
        if self.capacity > self.allocated {
            let want = u64::from(
                self.capacity
                    .max(self.allocated.saturating_mul(2))
                    .max(MIN_SLOTS),
            )
            .min(max)
            .max(u64::from(self.capacity)) as u32;
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("slots"),
                size: u64::from(want) * SLOT_BYTES as u64,
                usage: wgpu::BufferUsages::VERTEX
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            if let Some(old) = &self.buffer {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("slots grow"),
                });
                enc.copy_buffer_to_buffer(old, 0, &buffer, 0, old.size());
                // The copy is submitted before the write below, which runs at the next submit.
                queue.submit([enc.finish()]);
            }
            self.buffer = Some(buffer);
            self.allocated = want;
        }
        self.last_upload = 0;
        if let (Some((lo, hi)), Some(buffer)) = (dirty, &self.buffer) {
            let range = lo as usize * SLOT_BYTES..(hi as usize + 1) * SLOT_BYTES;
            self.last_upload = range.len() as u64;
            queue.write_buffer(buffer, range.start as u64, &self.mirror[range]);
        }
    }

    /// Bounds of the shown slots in absolute ENU metres, or None when nothing is shown.
    pub fn bounds(&self) -> Option<([f64; 3], [f64; 3])> {
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        for s in self.mirror.as_chunks::<SLOT_BYTES>().0 {
            if s[15] & FLAG_SHOWN == 0 {
                continue;
            }
            for a in 0..3 {
                let v = f64::from(f32::from_le_bytes(s[4 * a..4 * a + 4].try_into().unwrap()));
                min[a] = min[a].min(v);
                max[a] = max[a].max(v);
            }
        }
        let o = self.origin.to_array();
        (self.points > 0).then(|| {
            (
                [0, 1, 2].map(|a| o[a] + min[a]),
                [0, 1, 2].map(|a| o[a] + max[a]),
            )
        })
    }
}
