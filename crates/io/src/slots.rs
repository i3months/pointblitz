//! Slot diff messages (decision 0052): cumulus3d's live diff stream (v4) records, carried as they
//! are behind a 32-byte header.
//!
//! ```text
//! offset size  field
//!      0    4  magic "PBSD"
//!      4    4  version = 1            (u32 LE)
//!      8    4  seq                    (u32 LE)  event number of the stream
//!     12    4  n_add                  (u32 LE)
//!     16    4  n_move                 (u32 LE)
//!     20    4  n_remove               (u32 LE)
//!     24    4  capacity               (u32 LE)  highest slot id + 1 after this event
//!     28    4  reserved, zero
//!     32       the v4 pack, unchanged (little endian):
//!          n_add    × 20 B  u32 id | f32 x y z | u8 r g b | u8 zone
//!          n_move   × 20 B  u32 id | f32 x y z | u8 r g b | u8 0
//!          n_remove ×  4 B  u32 id
//! ```
//!
//! Every shown point owns a slot id; an add or a move writes the slot and shows it, a remove hides
//! it. Positions are f32 metres relative to the stream's origin (ENU). An id freed in an event is
//! not handed out again in the same event, so the order of the three sections does not matter.
//!
//! - [`encode`] / [`decode_header`] / [`validate`]: the header and the record checks.
//! - [`Diff`]: the three sections of a message as records.
//! - [`Fold`]: every message so far folded into one (the state a client joining late starts from).
//! - [`dir`] (cargo feature `stream`): reading a v4 `live/stream` directory.

pub const MAGIC: [u8; 4] = *b"PBSD";
pub const VERSION: u32 = 1;
pub const HEADER_LEN: usize = 32;
/// Bytes of an add record.
pub const ADD_BYTES: usize = 20;
/// Bytes of a move record.
pub const MOVE_BYTES: usize = 20;
/// Bytes of a remove record.
pub const REMOVE_BYTES: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SlotHeader {
    pub seq: u32,
    pub n_add: u32,
    pub n_move: u32,
    pub n_remove: u32,
    /// Highest slot id + 1 after this event: every id in the message is below it.
    pub capacity: u32,
}

impl SlotHeader {
    /// Pack bytes after the header: `20 (add + move) + 4 remove`.
    pub fn pack_len(&self) -> u64 {
        pack_len(self.n_add, self.n_move, self.n_remove)
    }
}

/// `20 (add + move) + 4 remove`, the v4 pack size. In u64, so a crafted header cannot overflow it on
/// a 32-bit target (wasm32).
pub fn pack_len(n_add: u32, n_move: u32, n_remove: u32) -> u64 {
    (u64::from(n_add) + u64::from(n_move)) * ADD_BYTES as u64
        + u64::from(n_remove) * REMOVE_BYTES as u64
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotError {
    TooShort,
    BadMagic,
    UnsupportedVersion(u32),
    ReservedNotZero,
    /// Message length does not match the record counts.
    BadLength {
        expected: u64,
        actual: u64,
    },
    /// A record names a slot at or above the message's capacity.
    IdOutOfRange {
        id: u32,
        capacity: u32,
    },
    /// `seq` u32::MAX: there would be no next seq.
    SeqOutOfRange,
}

impl std::fmt::Display for SlotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid slot message: {self:?}")
    }
}

impl std::error::Error for SlotError {}

/// Puts the header in front of a v4 pack. The pack is not parsed (decision 0052: no conversion);
/// only its length is checked against the counts.
pub fn encode(header: &SlotHeader, pack: &[u8]) -> Result<Vec<u8>, SlotError> {
    if pack.len() as u64 != header.pack_len() {
        return Err(SlotError::BadLength {
            expected: HEADER_LEN as u64 + header.pack_len(),
            actual: (HEADER_LEN + pack.len()) as u64,
        });
    }
    if header.seq == u32::MAX {
        return Err(SlotError::SeqOutOfRange);
    }
    let mut out = Vec::with_capacity(HEADER_LEN + pack.len());
    out.extend(MAGIC);
    for v in [
        VERSION,
        header.seq,
        header.n_add,
        header.n_move,
        header.n_remove,
        header.capacity,
        0,
    ] {
        out.extend(v.to_le_bytes());
    }
    debug_assert_eq!(out.len(), HEADER_LEN);
    out.extend_from_slice(pack);
    Ok(out)
}

fn u32_at(bytes: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap())
}

/// Checks the header and the message length; the records are not looked at ([`validate`] does).
pub fn decode_header(bytes: &[u8]) -> Result<SlotHeader, SlotError> {
    if bytes.len() < HEADER_LEN {
        return Err(SlotError::TooShort);
    }
    if bytes[..4] != MAGIC {
        return Err(SlotError::BadMagic);
    }
    let version = u32_at(bytes, 4);
    if version != VERSION {
        return Err(SlotError::UnsupportedVersion(version));
    }
    if u32_at(bytes, 28) != 0 {
        return Err(SlotError::ReservedNotZero);
    }
    let h = SlotHeader {
        seq: u32_at(bytes, 8),
        n_add: u32_at(bytes, 12),
        n_move: u32_at(bytes, 16),
        n_remove: u32_at(bytes, 20),
        capacity: u32_at(bytes, 24),
    };
    let expected = HEADER_LEN as u64 + h.pack_len();
    if bytes.len() as u64 != expected {
        return Err(SlotError::BadLength {
            expected,
            actual: bytes.len() as u64,
        });
    }
    if h.seq == u32::MAX {
        return Err(SlotError::SeqOutOfRange);
    }
    // From here every section offset is at most bytes.len(), so usize arithmetic cannot overflow.
    Ok(h)
}

/// Checks the header, the length and that every record's id is below the capacity.
pub fn validate(bytes: &[u8]) -> Result<SlotHeader, SlotError> {
    Diff::parse(bytes).map(|d| d.header)
}

/// One add or move record.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Record {
    pub id: u32,
    /// Metres relative to the stream origin.
    pub position: [f32; 3],
    /// sRGB.
    pub color: [u8; 3],
    /// Add: the zone that owns the point. Move: 0.
    pub zone: u8,
}

impl Record {
    /// Reads one 20-byte record.
    pub fn read(rec: &[u8]) -> Self {
        let f = |o: usize| f32::from_le_bytes(rec[o..o + 4].try_into().unwrap());
        Self {
            id: u32_at(rec, 0),
            position: [f(4), f(8), f(12)],
            color: [rec[16], rec[17], rec[18]],
            zone: rec[19],
        }
    }

    /// Writes the 20-byte record.
    pub fn write(&self, out: &mut Vec<u8>) {
        out.extend(self.id.to_le_bytes());
        for v in self.position {
            out.extend(v.to_le_bytes());
        }
        out.extend(self.color);
        out.push(self.zone);
    }
}

/// A validated message split into its three sections.
#[derive(Debug, Clone, Copy)]
pub struct Diff<'a> {
    pub header: SlotHeader,
    /// `n_add` × 20 B.
    pub adds: &'a [u8],
    /// `n_move` × 20 B.
    pub moves: &'a [u8],
    /// `n_remove` × 4 B.
    pub removes: &'a [u8],
}

impl<'a> Diff<'a> {
    /// Validates a message ([`decode_header`] plus id < capacity for every record).
    pub fn parse(bytes: &'a [u8]) -> Result<Self, SlotError> {
        let header = decode_header(bytes)?;
        let a = HEADER_LEN + header.n_add as usize * ADD_BYTES;
        let m = a + header.n_move as usize * MOVE_BYTES;
        let d = Self {
            header,
            adds: &bytes[HEADER_LEN..a],
            moves: &bytes[a..m],
            removes: &bytes[m..],
        };
        let check = |id: u32| {
            if id < header.capacity {
                Ok(())
            } else {
                Err(SlotError::IdOutOfRange {
                    id,
                    capacity: header.capacity,
                })
            }
        };
        for rec in d.adds.as_chunks::<ADD_BYTES>().0 {
            check(u32_at(rec, 0))?;
        }
        for rec in d.moves.as_chunks::<MOVE_BYTES>().0 {
            check(u32_at(rec, 0))?;
        }
        for id in d.remove_ids() {
            check(id)?;
        }
        Ok(d)
    }

    pub fn add_records(&self) -> impl Iterator<Item = Record> + 'a {
        self.adds
            .as_chunks::<ADD_BYTES>()
            .0
            .iter()
            .map(|r| Record::read(r))
    }

    pub fn move_records(&self) -> impl Iterator<Item = Record> + 'a {
        self.moves
            .as_chunks::<MOVE_BYTES>()
            .0
            .iter()
            .map(|r| Record::read(r))
    }

    pub fn remove_ids(&self) -> impl Iterator<Item = u32> + 'a {
        self.removes
            .as_chunks::<REMOVE_BYTES>()
            .0
            .iter()
            .map(|r| u32::from_le_bytes(*r))
    }
}

/// Highest add id + 1 in the first `n_add` records of a pack (0 without adds): with the capacity
/// before an event, gives the capacity after it.
pub fn add_capacity(pack: &[u8], n_add: u32) -> u32 {
    pack.as_chunks::<ADD_BYTES>()
        .0
        .iter()
        .take(n_add as usize)
        // Saturating: id u32::MAX cannot be below any capacity, so `validate` rejects it anyway.
        .map(|r| u32_at(r, 0).saturating_add(1))
        .max()
        .unwrap_or(0)
}

/// Every message so far folded into one: the shown slots as adds, in id order, with the last
/// message's seq and capacity. Applied to an empty scene it gives what the messages gave.
#[derive(Debug, Clone, Default)]
pub struct Fold {
    seq: u32,
    capacity: u32,
    /// Per slot: the add record (its zone kept across moves) and whether it is shown.
    slots: Vec<([u8; ADD_BYTES], bool)>,
    shown: usize,
}

impl Fold {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seq of the last message folded in (0 before any).
    pub fn seq(&self) -> u32 {
        self.seq
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Slots shown.
    pub fn points(&self) -> usize {
        self.shown
    }

    /// Folds in the next message (validated first; on an error nothing changes).
    pub fn apply(&mut self, bytes: &[u8]) -> Result<SlotHeader, SlotError> {
        let d = Diff::parse(bytes)?;
        let h = d.header;
        if h.capacity as usize > self.slots.len() {
            self.slots
                .resize(h.capacity as usize, ([0; ADD_BYTES], false));
        }
        for rec in d.adds.as_chunks::<ADD_BYTES>().0 {
            let s = &mut self.slots[u32_at(rec, 0) as usize];
            s.0.copy_from_slice(rec);
            self.shown += usize::from(!s.1);
            s.1 = true;
        }
        for rec in d.moves.as_chunks::<MOVE_BYTES>().0 {
            let s = &mut self.slots[u32_at(rec, 0) as usize];
            // Position and colour; the zone stays the one the add gave.
            s.0[..19].copy_from_slice(&rec[..19]);
            self.shown += usize::from(!s.1);
            s.1 = true;
        }
        for id in d.remove_ids() {
            let s = &mut self.slots[id as usize];
            self.shown -= usize::from(s.1);
            s.1 = false;
        }
        self.seq = h.seq;
        self.capacity = self.capacity.max(h.capacity);
        Ok(h)
    }

    /// The folded state as one message: an add per shown slot.
    pub fn message(&self) -> Vec<u8> {
        let mut pack = Vec::with_capacity(self.shown * ADD_BYTES);
        for (rec, shown) in &self.slots {
            if *shown {
                pack.extend_from_slice(rec);
            }
        }
        let header = SlotHeader {
            seq: self.seq,
            n_add: self.shown as u32,
            n_move: 0,
            n_remove: 0,
            capacity: self.capacity,
        };
        encode(&header, &pack).expect("pack length from the counts")
    }
}

#[cfg(feature = "stream")]
pub mod dir;

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A message from (id, x, rgb) adds, moves and removes.
    pub(crate) fn msg(
        seq: u32,
        capacity: u32,
        adds: &[(u32, f32, [u8; 3])],
        moves: &[(u32, f32, [u8; 3])],
        removes: &[u32],
    ) -> Vec<u8> {
        let mut pack = Vec::new();
        for (&(id, x, color), zone) in adds.iter().zip([7u8].into_iter().cycle()) {
            Record {
                id,
                position: [x, -x, 0.5 * x],
                color,
                zone,
            }
            .write(&mut pack);
        }
        for &(id, x, color) in moves {
            Record {
                id,
                position: [x, -x, 0.5 * x],
                color,
                zone: 0,
            }
            .write(&mut pack);
        }
        for id in removes {
            pack.extend(id.to_le_bytes());
        }
        let h = SlotHeader {
            seq,
            n_add: adds.len() as u32,
            n_move: moves.len() as u32,
            n_remove: removes.len() as u32,
            capacity,
        };
        encode(&h, &pack).unwrap()
    }

    #[test]
    fn header_round_trip_and_layout() {
        let m = msg(
            3,
            9,
            &[(8, 1.0, [1, 2, 3])],
            &[(0, 2.0, [4, 5, 6])],
            &[4, 5],
        );
        assert_eq!(&m[..4], b"PBSD");
        assert_eq!(m.len(), HEADER_LEN + 20 + 20 + 8);
        let h = validate(&m).unwrap();
        assert_eq!(
            h,
            SlotHeader {
                seq: 3,
                n_add: 1,
                n_move: 1,
                n_remove: 2,
                capacity: 9
            }
        );
        let d = Diff::parse(&m).unwrap();
        let a: Vec<Record> = d.add_records().collect();
        assert_eq!(a[0].id, 8);
        assert_eq!(a[0].position, [1.0, -1.0, 0.5]);
        assert_eq!((a[0].color, a[0].zone), ([1, 2, 3], 7));
        assert_eq!(d.move_records().next().unwrap().zone, 0);
        assert_eq!(d.remove_ids().collect::<Vec<_>>(), vec![4, 5]);
        // An empty event (cumulus3d's `final` without a pack) is a bare header.
        assert_eq!(validate(&msg(14, 9, &[], &[], &[])).unwrap().n_add, 0);
    }

    #[test]
    fn rejects_bad_messages() {
        let good = msg(1, 4, &[(3, 1.0, [9; 3])], &[], &[]);
        assert_eq!(decode_header(&good[..31]), Err(SlotError::TooShort));
        let mut bad = good.clone();
        bad[0] = b'X';
        assert_eq!(decode_header(&bad), Err(SlotError::BadMagic));
        let mut bad = good.clone();
        bad[4] = 2;
        assert_eq!(decode_header(&bad), Err(SlotError::UnsupportedVersion(2)));
        let mut bad = good.clone();
        bad[31] = 1;
        assert_eq!(decode_header(&bad), Err(SlotError::ReservedNotZero));
        assert!(matches!(
            decode_header(&good[..good.len() - 1]),
            Err(SlotError::BadLength { .. })
        ));
        // id 3 needs capacity 4.
        assert_eq!(
            validate(&msg(1, 3, &[(3, 1.0, [9; 3])], &[], &[])),
            Err(SlotError::IdOutOfRange { id: 3, capacity: 3 })
        );
        assert!(validate(&msg(1, 3, &[], &[], &[3])).is_err());
        assert!(validate(&msg(1, 3, &[], &[(5, 0.0, [0; 3])], &[])).is_err());
        // Counts that would overflow a 32-bit usize are only a length mismatch.
        let mut huge = good.clone();
        huge[12..24].copy_from_slice(&[0xff; 12]);
        assert!(matches!(
            decode_header(&huge),
            Err(SlotError::BadLength { expected, .. }) if expected > u64::from(u32::MAX)
        ));
        // seq u32::MAX has no successor.
        let mut last = msg(1, 4, &[], &[], &[]);
        last[8..12].copy_from_slice(&[0xff; 4]);
        assert_eq!(validate(&last), Err(SlotError::SeqOutOfRange));
        let h = SlotHeader {
            seq: u32::MAX,
            ..Default::default()
        };
        assert_eq!(encode(&h, &[]), Err(SlotError::SeqOutOfRange));
        // The pack must match the counts.
        let h = SlotHeader {
            n_add: 1,
            ..Default::default()
        };
        assert!(encode(&h, &[0; 19]).is_err());
    }

    #[test]
    fn fold_keeps_the_last_state_of_every_slot() {
        let mut f = Fold::new();
        f.apply(&msg(
            1,
            3,
            &[(0, 1.0, [1; 3]), (1, 2.0, [2; 3]), (2, 3.0, [3; 3])],
            &[],
            &[],
        ))
        .unwrap();
        // Move 1, remove 0, add 3; then 0 is reused by a later event.
        f.apply(&msg(2, 4, &[(3, 4.0, [4; 3])], &[(1, 5.0, [5; 3])], &[0]))
            .unwrap();
        f.apply(&msg(3, 4, &[(0, 6.0, [6; 3])], &[], &[2])).unwrap();
        assert_eq!((f.seq(), f.capacity(), f.points()), (3, 4, 3));
        let state = f.message();
        let d = Diff::parse(&state).unwrap();
        assert_eq!(
            (d.header.seq, d.header.n_move, d.header.n_remove),
            (3, 0, 0)
        );
        let got: Vec<(u32, f32, [u8; 3], u8)> = d
            .add_records()
            .map(|r| (r.id, r.position[0], r.color, r.zone))
            .collect();
        // Slot 1 keeps the zone of its add after the move.
        assert_eq!(
            got,
            vec![
                (0, 6.0, [6; 3], 7),
                (1, 5.0, [5; 3], 7),
                (3, 4.0, [4; 3], 7)
            ]
        );
        // A bad message changes nothing.
        assert!(f.apply(&msg(4, 2, &[(3, 0.0, [0; 3])], &[], &[])).is_err());
        assert_eq!(f.message(), state);
    }

    #[test]
    fn add_capacity_is_the_highest_add_id_plus_one() {
        let m = msg(
            1,
            10,
            &[(4, 0.0, [0; 3]), (9, 0.0, [0; 3])],
            &[(12, 0.0, [0; 3])],
            &[],
        );
        assert!(validate(&m).is_err()); // the move is out of range; add_capacity ignores moves
        assert_eq!(add_capacity(&m[HEADER_LEN..], 2), 10);
        assert_eq!(add_capacity(&[], 0), 0);
    }
}
