//! PointBlitz chunk format v1 (decisions 0008, 0022): what the server sends and the GPU consumes.
//!
//! ```text
//! offset size  field
//!      0    4  magic "PBCK"
//!      4    2  version = 1            (u16 LE)
//!      6    2  header length = 80     (u16 LE)
//!      8    4  generation             (u32 LE)  refined snapshots start a new generation
//!     12    4  index within generation (u32 LE)
//!     16    4  point count            (u32 LE)
//!     20    4  flags                  (u32 LE)  bit 0: last chunk of its generation
//!     24   24  origin x, y, z         (f64 LE, ENU metres)
//!     48   12  bbox min x, y, z       (f32 LE, relative to origin)
//!     60   12  bbox max x, y, z       (f32 LE, relative to origin)
//!     72    8  reserved, zero
//!     80  16·n points, interleaved:   position f32×3 (relative to origin) + color u8×4 (sRGB, a = 255)
//! ```
//!
//! The body after byte 80 is exactly a wgpu vertex buffer with stride 16
//! (`Float32x3` at 0, `Unorm8x4` at 12), so a client copies it to the GPU without parsing.

use crate::ply::Point;

pub const MAGIC: [u8; 4] = *b"PBCK";
pub const VERSION: u16 = 1;
pub const HEADER_LEN: usize = 80;
pub const POINT_STRIDE: usize = 16;
/// Set on the last chunk of a generation: the client may swap generations after it.
pub const FLAG_LAST_IN_GENERATION: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct ChunkHeader {
    pub generation: u32,
    pub index: u32,
    pub point_count: u32,
    pub flags: u32,
    pub origin: [f64; 3],
    pub bbox_min: [f32; 3],
    pub bbox_max: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkError {
    TooShort,
    BadMagic,
    UnsupportedVersion(u16),
    BadHeaderLength(u16),
    /// Body length does not match the point count.
    BadLength {
        expected: usize,
        actual: usize,
    },
}

impl std::fmt::Display for ChunkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid chunk: {self:?}")
    }
}

impl std::error::Error for ChunkError {}

/// Encodes points into one chunk. Positions are stored relative to `origin` (chosen per chunk so
/// f32 keeps centimetre precision far from the dataset origin — decision 0013).
pub fn encode(generation: u32, index: u32, flags: u32, points: &[Point]) -> Vec<u8> {
    let (min, max) = bounds(points);
    // Origin: bbox centre rounded to whole metres (stable, readable, exact in f64).
    let origin = [0, 1, 2].map(|i| ((min[i] + max[i]) / 2.0).round());
    let rel = |p: [f64; 3]| [0, 1, 2].map(|i| (p[i] - origin[i]) as f32);

    let mut out = Vec::with_capacity(HEADER_LEN + POINT_STRIDE * points.len());
    out.extend(MAGIC);
    out.extend(VERSION.to_le_bytes());
    out.extend((HEADER_LEN as u16).to_le_bytes());
    out.extend(generation.to_le_bytes());
    out.extend(index.to_le_bytes());
    out.extend(
        u32::try_from(points.len())
            .expect("chunk holds < 2^32 points")
            .to_le_bytes(),
    );
    out.extend(flags.to_le_bytes());
    for v in origin {
        out.extend(v.to_le_bytes());
    }
    let (bmin, bmax) = if points.is_empty() {
        ([0.0; 3], [0.0; 3])
    } else {
        (rel(min), rel(max))
    };
    for v in bmin.into_iter().chain(bmax) {
        out.extend(v.to_le_bytes());
    }
    out.extend([0u8; 8]);
    debug_assert_eq!(out.len(), HEADER_LEN);

    for p in points {
        for v in rel(p.position.map(f64::from)) {
            out.extend(v.to_le_bytes());
        }
        out.extend([p.color[0], p.color[1], p.color[2], 255]);
    }
    out
}

fn bounds(points: &[Point]) -> ([f64; 3], [f64; 3]) {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for p in points {
        for i in 0..3 {
            let v = f64::from(p.position[i]);
            min[i] = min[i].min(v);
            max[i] = max[i].max(v);
        }
    }
    if points.is_empty() {
        ([0.0; 3], [0.0; 3])
    } else {
        (min, max)
    }
}

/// Validates a chunk and returns its header.
pub fn decode_header(bytes: &[u8]) -> Result<ChunkHeader, ChunkError> {
    if bytes.len() < HEADER_LEN {
        return Err(ChunkError::TooShort);
    }
    if bytes[..4] != MAGIC {
        return Err(ChunkError::BadMagic);
    }
    let u16_at = |o: usize| u16::from_le_bytes([bytes[o], bytes[o + 1]]);
    let u32_at = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
    let f32_at = |o: usize| f32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
    let f64_at = |o: usize| f64::from_le_bytes(bytes[o..o + 8].try_into().unwrap());
    let version = u16_at(4);
    if version != VERSION {
        return Err(ChunkError::UnsupportedVersion(version));
    }
    if usize::from(u16_at(6)) != HEADER_LEN {
        return Err(ChunkError::BadHeaderLength(u16_at(6)));
    }
    let point_count = u32_at(16);
    let expected = (point_count as usize)
        .checked_mul(POINT_STRIDE)
        .and_then(|b| b.checked_add(HEADER_LEN))
        .ok_or(ChunkError::BadLength {
            expected: usize::MAX,
            actual: bytes.len(),
        })?;
    if bytes.len() != expected {
        return Err(ChunkError::BadLength {
            expected,
            actual: bytes.len(),
        });
    }
    Ok(ChunkHeader {
        generation: u32_at(8),
        index: u32_at(12),
        point_count,
        flags: u32_at(20),
        origin: [f64_at(24), f64_at(32), f64_at(40)],
        bbox_min: [f32_at(48), f32_at(52), f32_at(56)],
        bbox_max: [f32_at(60), f32_at(64), f32_at(68)],
    })
}

/// The vertex data of a validated chunk (stride 16), ready to copy into a GPU buffer.
pub fn vertex_bytes(bytes: &[u8]) -> &[u8] {
    &bytes[HEADER_LEN..]
}

/// Splits a complete PLY file into chunks of at most `max_points` points for one generation.
/// The last chunk carries [`FLAG_LAST_IN_GENERATION`].
pub fn ply_to_chunks(
    ply: &[u8],
    generation: u32,
    max_points: usize,
) -> Result<Vec<Vec<u8>>, crate::PlyError> {
    assert!(max_points > 0);
    let all: Vec<Point> = crate::ply::points(ply)?.collect();
    let parts: Vec<&[Point]> = if all.is_empty() {
        vec![&all[..]]
    } else {
        all.chunks(max_points).collect()
    };
    let last = parts.len() - 1;
    Ok(parts
        .iter()
        .enumerate()
        .map(|(i, part)| {
            let flags = if i == last {
                FLAG_LAST_IN_GENERATION
            } else {
                0
            };
            encode(generation, i as u32, flags, part)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(n: usize, offset: f32) -> Vec<Point> {
        (0..n)
            .map(|i| Point {
                position: [offset + i as f32 * 0.25, -(i as f32), 10.0],
                color: [i as u8, 2, 3],
            })
            .collect()
    }

    #[test]
    fn layout_is_header_80_plus_16_per_point() {
        let c = encode(3, 1, FLAG_LAST_IN_GENERATION, &pts(5, 0.0));
        assert_eq!(c.len(), 80 + 16 * 5);
        assert_eq!(&c[..4], b"PBCK");
        let h = decode_header(&c).unwrap();
        assert_eq!(
            (h.generation, h.index, h.point_count, h.flags),
            (3, 1, 5, 1)
        );
        // First point: position relative to origin, then RGBA.
        let v = vertex_bytes(&c);
        let x = f32::from_le_bytes(v[0..4].try_into().unwrap());
        assert_eq!(f64::from(x) + h.origin[0], 0.0);
        assert_eq!(&v[12..16], &[0, 2, 3, 255]);
    }

    #[test]
    fn positions_round_trip_far_from_origin() {
        // 5 km away: relative storage keeps the exact f32 inputs.
        let input = pts(100, 5000.0);
        let c = encode(0, 0, 0, &input);
        let h = decode_header(&c).unwrap();
        for (i, rec) in vertex_bytes(&c).as_chunks::<16>().0.iter().enumerate() {
            let f = |o: usize| f32::from_le_bytes(rec[o..o + 4].try_into().unwrap());
            let back = [0, 1, 2].map(|k| (f64::from(f(k * 4)) + h.origin[k]) as f32);
            assert_eq!(back, input[i].position);
        }
        assert!(h.bbox_min[0] <= 0.0 && h.bbox_max[0] >= 0.0);
    }

    #[test]
    fn rejects_bad_chunks() {
        let c = encode(0, 0, 0, &pts(2, 0.0));
        assert_eq!(decode_header(&c[..79]), Err(ChunkError::TooShort));
        let mut bad = c.clone();
        bad[0] = b'X';
        assert_eq!(decode_header(&bad), Err(ChunkError::BadMagic));
        let mut v2 = c.clone();
        v2[4] = 2;
        assert_eq!(decode_header(&v2), Err(ChunkError::UnsupportedVersion(2)));
        assert!(matches!(
            decode_header(&c[..c.len() - 1]),
            Err(ChunkError::BadLength { .. })
        ));
    }

    #[test]
    fn empty_chunk_is_valid() {
        let c = encode(9, 0, FLAG_LAST_IN_GENERATION, &[]);
        assert_eq!(decode_header(&c).unwrap().point_count, 0);
        assert_eq!(c.len(), 80);
    }

    #[test]
    fn ply_splits_into_flagged_chunks() {
        let mut ply = b"ply\nformat binary_little_endian 1.0\nelement vertex 10\n\
             property float x\nproperty float y\nproperty float z\n\
             property uchar red\nproperty uchar green\nproperty uchar blue\nend_header\n"
            .to_vec();
        for i in 0..10u8 {
            for v in [f32::from(i), 0.0, 0.0] {
                ply.extend(v.to_le_bytes());
            }
            ply.extend([i, i, i]);
        }
        let chunks = ply_to_chunks(&ply, 4, 4).unwrap();
        let heads: Vec<_> = chunks.iter().map(|c| decode_header(c).unwrap()).collect();
        assert_eq!(
            heads.iter().map(|h| h.point_count).collect::<Vec<_>>(),
            [4, 4, 2]
        );
        assert_eq!(heads.iter().map(|h| h.flags).collect::<Vec<_>>(), [0, 0, 1]);
        assert!(heads.iter().all(|h| h.generation == 4));
    }
}
