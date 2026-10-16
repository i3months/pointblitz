//! Real-data check against the theater30 stable-display stream (cumulus3d 0.4.0 v4, decision 0052).
//!
//! The stream is not in the repository (SPEC §8.3, bench/data/README.md). Set
//! `PB_THEATER30_STREAM=<…/live/stream>` to run; without it the test passes after printing that it
//! was skipped, so CI stays green.
//!
//! `cargo test -p pointblitz-io --features stream --release --test theater30 -- --nocapture`

#![cfg(feature = "stream")]

use pointblitz_io::slots::dir::StreamDir;
use pointblitz_io::slots::{Diff, Fold, HEADER_LEN, validate};
use std::time::Instant;

#[test]
fn theater30_events_keep_their_invariants() {
    let Ok(dir) = std::env::var("PB_THEATER30_STREAM") else {
        eprintln!("PB_THEATER30_STREAM not set — skipped");
        return;
    };
    let mut s = StreamDir::open(&dir).unwrap();
    let m = s.manifest().clone();
    assert_eq!(m.version, 4);
    assert_eq!(m.frame, "enu");
    assert_eq!(m.events.len(), 14);
    assert_eq!(m.capacity, 2_881_929);

    let t = Instant::now();
    let all = s.messages().unwrap();
    let t_read = t.elapsed();
    let mut fold = Fold::new();
    let mut prev = 0u64;
    let mut total = 0u64;
    let mut capacity = 0;
    for (e, msg) in &all {
        let h = Diff::parse(msg).unwrap().header;
        // points = previous + add − remove; bytes = 20 (add + move) + 4 remove (decision 0052).
        assert_eq!(
            prev + u64::from(h.n_add) - u64::from(h.n_remove),
            e.points,
            "event {}",
            e.seq
        );
        let bytes = 20 * (u64::from(h.n_add) + u64::from(h.n_move)) + 4 * u64::from(h.n_remove);
        assert_eq!(bytes, e.bytes, "event {}", e.seq);
        assert_eq!(msg.len() as u64, HEADER_LEN as u64 + e.bytes);
        assert!(h.capacity >= capacity, "capacity never shrinks");
        capacity = h.capacity;
        let t = Instant::now();
        fold.apply(msg).unwrap();
        // The folded state shows exactly the event's points.
        assert_eq!(fold.points() as u64, e.points, "event {}", e.seq);
        println!(
            "seq {:2} {:8} add {:7} move {:7} remove {:7} → {:9} pts, {:10} B, capacity {:9}, fold {:.1} ms",
            e.seq,
            e.kind,
            h.n_add,
            h.n_move,
            h.n_remove,
            e.points,
            msg.len(),
            h.capacity,
            t.elapsed().as_secs_f64() * 1e3
        );
        prev = e.points;
        total += e.bytes;
    }
    assert_eq!(capacity, m.capacity);
    let t = Instant::now();
    let state = fold.message();
    let h = validate(&state).unwrap();
    println!(
        "read + validate {:.1} ms; diff total {total} B; state {} B ({} pts) built in {:.1} ms",
        t_read.as_secs_f64() * 1e3,
        state.len(),
        h.n_add,
        t.elapsed().as_secs_f64() * 1e3
    );
    assert_eq!(u64::from(h.n_add), all.last().unwrap().0.points);
    assert_eq!(h.seq, 14);
    // The state applied to an empty fold gives the same state again.
    let mut again = Fold::new();
    again.apply(&state).unwrap();
    assert_eq!(again.message(), state);
}
