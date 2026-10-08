//! Server tick phase lock (decision 0042): moves the server's tick so frames reach the client in
//! the middle of its display cycle, away from the refresh boundary where a little arrival jitter
//! turns into an empty cycle next to a double one (diagnostics P4.3).
//!
//! The client reports `err_ms = (0.65 − φ̄) × P` (first after 10 frames, then every 30; φ̄: circular
//! mean of where decoded frames land in its display cycle of period P; 0.65 = before the next
//! rendering update with ~6 ms to spare, decision 0042 tuning). Each report adds a PI correction to a pending
//! shift, which is paid out at most `MAX_STEP_MS` per tick (positive = the next tick later).
//! Times are plain milliseconds so the loop can be simulated in tests.

const KP: f64 = 0.25;
/// Gain of the first `ACQUIRE_REPORTS` reports, to pull the phase in quickly after connecting.
const KP_ACQUIRE: f64 = 0.75;
const ACQUIRE_REPORTS: u32 = 3;
const KI: f64 = 0.05;
const MAX_STEP_MS: f64 = 1.0;
const MAX_INTEGRAL_MS: f64 = 4.0;
const STALE_MS: f64 = 2000.0;

#[derive(Debug, Default)]
pub struct PhaseLock {
    pending: f64,
    integral: f64,
    last_report: Option<f64>,
    reports: u32,
}

/// What one report did, for the server log.
#[derive(Debug, Clone, Copy)]
pub struct Report {
    pub err_ms: f64,
    pub added_ms: f64,
    pub integral_ms: f64,
}

impl PhaseLock {
    /// A client report. Rejects non-finite errors and errors beyond a period (+0.5 ms): the error is
    /// not wrapped, so the correction moves the phase away from the refresh boundary, never across it.
    pub fn report(&mut self, err_ms: f64, period_ms: f64, now_ms: f64) -> Option<Report> {
        if !err_ms.is_finite() || !period_ms.is_finite() || err_ms.abs() > period_ms + 0.5 {
            return None;
        }
        self.integral = (self.integral + KI * err_ms).clamp(-MAX_INTEGRAL_MS, MAX_INTEGRAL_MS);
        let kp = if self.reports < ACQUIRE_REPORTS {
            KP_ACQUIRE
        } else {
            KP
        };
        self.reports += 1;
        let added = kp * err_ms + self.integral;
        self.pending += added;
        self.last_report = Some(now_ms);
        Some(Report {
            err_ms,
            added_ms: added,
            integral_ms: self.integral,
        })
    }

    /// The shift to add to the next tick deadline (ms, at most ±`MAX_STEP_MS`). Without a report
    /// for `STALE_MS`, the lock lets go and the ticks are plain fixed-rate again.
    pub fn tick_shift(&mut self, now_ms: f64) -> f64 {
        if self.last_report.is_none_or(|t| now_ms - t > STALE_MS) {
            *self = Self::default();
            return 0.0;
        }
        let step = self.pending.clamp(-MAX_STEP_MS, MAX_STEP_MS);
        self.pending -= step;
        step
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic uniform noise in [-1, 1).
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
        }
    }

    struct Outcome {
        mean_phase: f64,
        settle_frames: usize,
        near_edge: f64,
        min_interval: f64,
        max_interval: f64,
        shift_sd: f64,
    }

    const TARGET: f64 = 0.65;

    /// Server ticks at 60 Hz with the lock, frames arrive 10 ms after their tick ± `jitter`,
    /// the display runs at `display_hz` starting at `display_offset`; reports every 30 frames.
    fn simulate(display_hz: f64, display_offset: f64, jitter: f64, frames: usize) -> Outcome {
        let tick = 1000.0 / 60.0;
        let period = 1000.0 / display_hz;
        let mut lock = PhaseLock::default();
        let mut rng = Lcg(42);
        let mut t = 0.0;
        let (mut c, mut s) = (0.0, 0.0);
        let mut phases = Vec::new();
        let mut intervals = Vec::new();
        let mut shifts = Vec::new();
        for n in 0..frames {
            let arrival = t + 10.0 + jitter * rng.next();
            let phi = ((arrival - display_offset) / period).rem_euclid(1.0);
            phases.push(phi);
            let a = std::f64::consts::TAU * phi;
            c += a.cos();
            s += a.sin();
            if n + 1 == 10 || (n + 1 > 10 && (n + 1 - 10) % 30 == 0) {
                let mean = (s.atan2(c) / std::f64::consts::TAU).rem_euclid(1.0);
                lock.report((TARGET - mean) * period, period, t);
                (c, s) = (0.0, 0.0);
            }
            let shift = lock.tick_shift(t);
            shifts.push(shift);
            intervals.push(tick + shift);
            t += tick + shift;
        }
        // Settled: the first frame after which no frame lands within 0.1 of the boundary (φ = 0 / 1, where the risk is — decision 0042 tuning).
        let settle_frames = phases
            .iter()
            .rposition(|p| *p < 0.1 || *p > 0.9)
            .map_or(0, |i| i + 1);
        let late = &phases[frames / 2..];
        let (lc, ls) = late.iter().fold((0.0, 0.0), |(c, s), p| {
            let a = std::f64::consts::TAU * p;
            (c + a.cos(), s + a.sin())
        });
        let settled = &shifts[frames / 2..];
        let m = settled.iter().sum::<f64>() / settled.len() as f64;
        let sd =
            (settled.iter().map(|x| (x - m).powi(2)).sum::<f64>() / settled.len() as f64).sqrt();
        let iv = &intervals[frames / 2..];
        Outcome {
            mean_phase: (ls.atan2(lc) / std::f64::consts::TAU).rem_euclid(1.0),
            settle_frames,
            near_edge: late.iter().filter(|p| **p < 0.1 || **p > 0.9).count() as f64
                / late.len() as f64,
            min_interval: iv.iter().copied().fold(f64::INFINITY, f64::min),
            max_interval: iv.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            shift_sd: sd,
        }
    }

    #[test]
    fn converges_to_mid_cycle_and_stays_there() {
        // Decision 0042: equal and slightly different clocks, starting at and away from the edge.
        for display_hz in [60.0, 59.94] {
            for offset in [0.0, 3.0, 8.0, 12.0, 16.3] {
                let o = simulate(display_hz, offset, 2.5, 6000);
                let what = format!("{display_hz} Hz, offset {offset} ms");
                eprintln!(
                    "{what}: phase {:.3}, settled after {} frames, near edge {:.4}, interval {:.2}–{:.2} ms, shift sd {:.3}",
                    o.mean_phase,
                    o.settle_frames,
                    o.near_edge,
                    o.min_interval,
                    o.max_interval,
                    o.shift_sd
                );
                assert!(
                    (o.mean_phase - TARGET).abs() < 0.1,
                    "{what}: mean phase {}",
                    o.mean_phase
                );
                assert!(o.near_edge < 0.01, "{what}: {} near the edge", o.near_edge);
                assert!(
                    o.settle_frames < 60,
                    "{what}: settled after {} frames",
                    o.settle_frames
                );
                assert!(
                    o.min_interval >= 15.66 && o.max_interval <= 17.67,
                    "{what}: tick interval {}–{}",
                    o.min_interval,
                    o.max_interval
                );
                assert!(o.shift_sd < 0.5, "{what}: shift sd {}", o.shift_sd);
            }
        }
    }

    #[test]
    fn rejects_bad_reports_and_lets_go_when_reports_stop() {
        let mut lock = PhaseLock::default();
        assert!(lock.report(f64::NAN, 16.7, 0.0).is_none());
        assert!(lock.report(20.0, 16.7, 0.0).is_none()); // beyond a period
        assert_eq!(lock.tick_shift(10.0), 0.0); // no accepted report yet
        assert!(lock.report(8.0, 16.7, 100.0).is_some());
        assert_eq!(lock.tick_shift(116.7), 1.0); // capped per tick
        assert_eq!(lock.tick_shift(3000.0), 0.0); // stale: fixed ticks again
        assert_eq!(lock.tick_shift(3016.7), 0.0);
    }
}
