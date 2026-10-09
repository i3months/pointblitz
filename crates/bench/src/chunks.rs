//! Chunk delivery for PointBlitz clients (SPEC §3.3, decision 0026).
//!
//! `GET /chunks/<seq>?have=<generation>.<points>` streams snapshot `seq` as chunks. The conversion
//! starts when the snapshot is announced and is kept in memory (decision 0050; before, on every
//! request — 0026); it still runs inside the client's measured window (event received → presented),
//! the same place three.js pays for parsing.
//!
//! - **Full delivery**: every point, as a new generation numbered by the snapshot's `seq`.
//! - **Delta delivery**: only the points after the client's `points`, appended to its generation.
//!   Sent when the snapshot is a preview and the client holds exactly the previous snapshot — that
//!   is, every snapshot after its generation started is a preview that only appends points.
//!
//! Generation numbers are snapshot numbers, so they only grow and a client that joins late (or
//! misses a snapshot) simply gets a full delivery.

use crate::replay::{Event, Kind};
use pointblitz_io::convert::Step;
pub use pointblitz_io::convert::{Plan, parse_have};

/// The events as the shared delivery rules see them (`pointblitz_io::convert`, decision 0050).
pub fn steps(events: &[Event]) -> Vec<Step> {
    events
        .iter()
        .map(|e| Step {
            seq: e.seq,
            preview: e.kind == Kind::Preview,
            points: e.points,
        })
        .collect()
}

/// Chooses the delivery for `events[idx]`. `appends(k)` says whether snapshot `k` (k ≥ 1) holds
/// every record of snapshot `k − 1` unchanged at its start (checked on the bytes by the caller).
pub fn plan(
    events: &[Event],
    idx: usize,
    have: Option<(u32, usize)>,
    appends: impl FnMut(usize) -> bool,
) -> Plan {
    pointblitz_io::convert::plan(&steps(events), idx, have, appends)
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
