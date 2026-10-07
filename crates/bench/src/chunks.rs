//! Chunk delivery for PointBlitz clients (SPEC §3.3, decision 0026).
//!
//! `GET /chunks/<seq>?have=<generation>.<points>` converts snapshot `seq` to chunks on request and
//! streams them as they are encoded. The conversion runs inside the client's measured window
//! (event received → presented), the same place three.js pays for parsing.
//!
//! - **Full delivery**: every point, as a new generation numbered by the snapshot's `seq`.
//! - **Delta delivery**: only the points after the client's `points`, appended to its generation.
//!   Sent when the snapshot is a preview and the client holds exactly the previous snapshot — that
//!   is, every snapshot after its generation started is a preview that only appends points.
//!
//! Generation numbers are snapshot numbers, so they only grow and a client that joins late (or
//! misses a snapshot) simply gets a full delivery.

use crate::replay::{Event, Kind};

/// Largest chunk (decision 0022).
pub const MAX_POINTS: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    Full { generation: u32 },
    Delta { generation: u32, skip: usize },
}

/// Parses `have=<generation>.<points>`.
pub fn parse_have(v: &str) -> Option<(u32, usize)> {
    let (g, n) = v.split_once('.')?;
    Some((g.parse().ok()?, n.parse().ok()?))
}

/// Chooses the delivery for `events[idx]`. `appends(k)` says whether snapshot `k` (k ≥ 1) holds
/// every record of snapshot `k − 1` unchanged at its start (checked on the bytes by the caller).
pub fn plan(
    events: &[Event],
    idx: usize,
    have: Option<(u32, usize)>,
    mut appends: impl FnMut(usize) -> bool,
) -> Plan {
    let e = &events[idx];
    let full = Plan::Full { generation: e.seq };
    let Some((generation, points)) = have else {
        return full;
    };
    if e.kind != Kind::Preview || idx == 0 || events[idx - 1].points != points {
        return full;
    }
    // The client's generation must have started at a snapshot from which every later snapshot up
    // to this one only appended points.
    let Some(start) = events.iter().position(|x| x.seq == generation) else {
        return full;
    };
    if start >= idx {
        return full;
    }
    let chain = (start + 1..=idx).all(|k| events[k].kind == Kind::Preview && appends(k));
    if chain {
        Plan::Delta {
            generation,
            skip: points,
        }
    } else {
        full
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(seq: u32, kind: Kind, points: usize) -> Event {
        Event {
            seq,
            time_s: f64::from(seq),
            kind,
            region: 0,
            file: format!("e{seq}"),
            bytes: 0,
            points,
        }
    }

    fn flight() -> Vec<Event> {
        vec![
            ev(1, Kind::Preview, 100),
            ev(2, Kind::Refined, 200),
            ev(3, Kind::Preview, 230),
            ev(4, Kind::Preview, 260),
            ev(5, Kind::Refined, 400),
        ]
    }

    #[test]
    fn preview_after_what_the_client_holds_is_a_delta() {
        let ev = flight();
        assert_eq!(
            plan(&ev, 2, Some((2, 200)), |_| true),
            Plan::Delta {
                generation: 2,
                skip: 200
            }
        );
        // Two previews in a row extend the same generation.
        assert_eq!(
            plan(&ev, 3, Some((2, 230)), |_| true),
            Plan::Delta {
                generation: 2,
                skip: 230
            }
        );
    }

    #[test]
    fn everything_else_is_a_full_generation_named_by_the_snapshot() {
        let ev = flight();
        assert_eq!(plan(&ev, 0, None, |_| true), Plan::Full { generation: 1 });
        // Refined: always full.
        assert_eq!(
            plan(&ev, 4, Some((2, 260)), |_| true),
            Plan::Full { generation: 5 }
        );
        // Late join / missed snapshot: wrong point count.
        assert_eq!(
            plan(&ev, 3, Some((2, 200)), |_| true),
            Plan::Full { generation: 4 }
        );
        // Not an append on the bytes.
        assert_eq!(
            plan(&ev, 2, Some((2, 200)), |_| false),
            Plan::Full { generation: 3 }
        );
        // A link earlier in the chain is not an append.
        assert_eq!(
            plan(&ev, 3, Some((2, 230)), |k| k != 2),
            Plan::Full { generation: 4 }
        );
        // Unknown or future generation.
        assert_eq!(
            plan(&ev, 2, Some((9, 200)), |_| true),
            Plan::Full { generation: 3 }
        );
        assert_eq!(
            plan(&ev, 2, Some((3, 200)), |_| true),
            Plan::Full { generation: 3 }
        );
    }

    #[test]
    fn parses_have() {
        assert_eq!(parse_have("2.336118"), Some((2, 336_118)));
        assert_eq!(parse_have("2"), None);
        assert_eq!(parse_have("x.1"), None);
    }
}
