//! GPU tests of the slot scene (decision 0052): points appear, move and disappear as messages are
//! applied, in seq order, and draw in the same pass as chunk scenes; upload method (b), scatter,
//! leaves the same slot bytes and the same captures as method (a), the mirror. Skipped (pass with a
//! message) when no GPU adapter is available, e.g. in CI, or with `PB_NO_GPU=1`.
//!
//! `theater30_replace_events_apply_in_one_frame` and `theater30_scatter_matches_mirror` also need
//! the real stream (`PB_THEATER30_STREAM=<…/live/stream>`, bench/data/README.md); the first prints
//! apply times: `cargo test -p pointblitz-core --release --test slots -- --nocapture`

use pointblitz_core::{Applied, Camera, Headless, Scene, SlotScene, UploadMethod};
use pointblitz_io::Point;
use pointblitz_io::chunk::{FLAG_LAST_IN_GENERATION, encode as encode_chunk};
use pointblitz_io::slots::{Record, SlotHeader, encode};

const W: u32 = 320;
const H: u32 = 180;

/// One GPU test at a time: with many devices created and polled at once from parallel test threads,
/// the NVIDIA Vulkan driver (V100, Linux) sometimes never returns (seen 1 run in 3; every test passes
/// alone and the whole file passes with `--test-threads=1`). The guard keeps the GPU part serial.
static GPU: std::sync::Mutex<()> = std::sync::Mutex::new(());

type Gpu = (
    wgpu::Device,
    wgpu::Queue,
    std::sync::MutexGuard<'static, ()>,
);

fn gpu() -> Option<Gpu> {
    let guard = GPU.lock().unwrap_or_else(|e| e.into_inner());
    match pointblitz_core::headless::device() {
        Some((d, q, info)) => {
            eprintln!("adapter: {} ({:?})", info.name, info.backend);
            Some((d, q, guard))
        }
        None => {
            eprintln!("no GPU adapter — skipped");
            None
        }
    }
}

/// A message from (id, position, rgb) adds and moves and the ids removed.
fn msg(
    seq: u32,
    capacity: u32,
    adds: &[(u32, [f32; 3], [u8; 3])],
    moves: &[(u32, [f32; 3], [u8; 3])],
    removes: &[u32],
) -> Vec<u8> {
    let mut pack = Vec::new();
    for (list, zone) in [(adds, 1), (moves, 0)] {
        for &(id, position, color) in list {
            Record {
                id,
                position,
                color,
                zone,
            }
            .write(&mut pack);
        }
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

fn pixel(img: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    img[i..i + 4].try_into().unwrap()
}

fn at(cam: &Camera, p: [f64; 3]) -> (u32, u32) {
    let [x, y] = cam.project_px(p, W, H).expect("in front of camera");
    (x as u32, y as u32)
}

fn lit(img: &[u8]) -> usize {
    img.as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[..3] != [0; 3])
        .count()
}

fn looking_north() -> Camera {
    Camera::new([0.0, -10.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 50.0)
}

#[test]
fn points_appear_move_and_disappear() {
    let Some((device, queue, _gpu)) = gpu() else {
        return;
    };
    let cam = looking_north();
    let mut hl = Headless::new(&device, [W, H]);
    let mut s = SlotScene::new([0.0; 3]);
    let capture =
        |hl: &mut Headless, s: &SlotScene| hl.capture_with_slots(&device, &queue, &cam, &[], &[s]);
    // Nothing applied: nothing drawn.
    assert_eq!(lit(&capture(&mut hl, &s)), 0);

    let a = [1.0f32, 0.0, 0.0];
    let b = [-1.0f32, 0.0, 0.5];
    let c = [0.0f32, 0.0, -0.8];
    let r = s.apply(
        &device,
        &queue,
        &msg(1, 2, &[(0, a, [255, 0, 0]), (1, b, [0, 255, 0])], &[], &[]),
    );
    assert_eq!(r.unwrap(), Applied::Shown(vec![1]));
    assert_eq!((s.points(), s.capacity()), (2, 2));
    let img = capture(&mut hl, &s);
    let (ax, ay) = at(&cam, a.map(f64::from));
    let (bx, by) = at(&cam, b.map(f64::from));
    let (cx, cy) = at(&cam, c.map(f64::from));
    assert_eq!(pixel(&img, ax, ay), [255, 0, 0, 255]);
    assert_eq!(pixel(&img, bx, by), [0, 255, 0, 255]);
    assert!((2..=8).contains(&lit(&img)), "2 px points: {}", lit(&img));

    // Move slot 0 to c (and recolour), remove slot 1, add slot 2 at a in blue.
    let r = s.apply(
        &device,
        &queue,
        &msg(2, 3, &[(2, a, [0, 0, 255])], &[(0, c, [255, 255, 0])], &[1]),
    );
    assert_eq!(r.unwrap(), Applied::Shown(vec![2]));
    assert_eq!((s.points(), s.capacity()), (2, 3));
    let img = capture(&mut hl, &s);
    assert_eq!(pixel(&img, cx, cy), [255, 255, 0, 255], "moved");
    assert_eq!(pixel(&img, ax, ay), [0, 0, 255, 255], "added in a new slot");
    assert_eq!(pixel(&img, bx, by), [0, 0, 0, 255], "removed");

    // Remove everything: the scene still draws `capacity` instances, all clipped.
    s.apply(&device, &queue, &msg(3, 3, &[], &[], &[0, 2]))
        .unwrap();
    assert_eq!(s.points(), 0);
    assert_eq!(lit(&capture(&mut hl, &s)), 0);
}

#[test]
fn messages_apply_in_seq_order() {
    let Some((device, queue, _gpu)) = gpu() else {
        return;
    };
    let cam = looking_north();
    let mut hl = Headless::new(&device, [W, H]);
    let mut s = SlotScene::new([0.0; 3]);
    let p = [0.5f32, 0.0, 0.2];
    let (px, py) = at(&cam, p.map(f64::from));
    s.apply(
        &device,
        &queue,
        &msg(1, 1, &[(0, p, [10, 20, 30])], &[], &[]),
    )
    .unwrap();
    // 3 before 2: held, the screen still shows 1.
    let three = msg(3, 1, &[], &[(0, p, [200, 100, 50])], &[]);
    assert_eq!(s.apply(&device, &queue, &three).unwrap(), Applied::Held);
    assert_eq!((s.seq(), s.held()), (Some(1), 1));
    let img = hl.capture_with_slots(&device, &queue, &cam, &[], &[&s]);
    assert_eq!(pixel(&img, px, py), [10, 20, 30, 255]);
    // 2 arrives: 2 and 3 are applied together, in one upload.
    let two = msg(2, 1, &[], &[(0, p, [1, 2, 3])], &[]);
    assert_eq!(
        s.apply(&device, &queue, &two).unwrap(),
        Applied::Shown(vec![2, 3])
    );
    assert_eq!((s.seq(), s.held()), (Some(3), 0));
    let img = hl.capture_with_slots(&device, &queue, &cam, &[], &[&s]);
    assert_eq!(pixel(&img, px, py), [200, 100, 50, 255]);
    // Old and bad messages change nothing.
    assert_eq!(s.apply(&device, &queue, &two).unwrap(), Applied::Stale);
    assert!(
        s.apply(&device, &queue, &msg(4, 1, &[(1, p, [0; 3])], &[], &[]))
            .is_err()
    );
    assert_eq!(s.seq(), Some(3));
    // A folded state of any seq is taken through apply_state.
    let state = msg(9, 1, &[(0, p, [7, 7, 7])], &[], &[]);
    assert_eq!(
        s.apply_state(&device, &queue, &state).unwrap(),
        Applied::Shown(vec![9])
    );
    let img = hl.capture_with_slots(&device, &queue, &cam, &[], &[&s]);
    assert_eq!(pixel(&img, px, py), [7, 7, 7, 255]);
}

#[test]
fn a_new_scene_starts_only_at_seq_1_or_a_folded_state() {
    let Some((device, queue, _gpu)) = gpu() else {
        return;
    };
    let cam = looking_north();
    let mut hl = Headless::new(&device, [W, H]);
    let a = [1.0f32, 0.0, 0.0];
    let b = [-1.0f32, 0.0, 0.0];
    let (ax, ay) = at(&cam, a.map(f64::from));
    let (bx, by) = at(&cam, b.map(f64::from));
    // Event 1 could not be fetched: event 2 must not become the start (event 1's points would
    // never appear).
    let mut s = SlotScene::new([0.0; 3]);
    let two = msg(2, 2, &[(1, b, [0, 0, 255])], &[], &[]);
    assert_eq!(s.apply(&device, &queue, &two).unwrap(), Applied::Held);
    assert_eq!((s.seq(), s.points()), (None, 0));
    // The client falls back to the folded state through 1: it starts the scene and the held event
    // 2 follows in the same upload.
    let state1 = msg(1, 1, &[(0, a, [255, 0, 0])], &[], &[]);
    assert_eq!(
        s.apply_state(&device, &queue, &state1).unwrap(),
        Applied::Shown(vec![1, 2])
    );
    let img = hl.capture_with_slots(&device, &queue, &cam, &[], &[&s]);
    assert_eq!(pixel(&img, ax, ay), [255, 0, 0, 255]);
    assert_eq!(pixel(&img, bx, by), [0, 0, 255, 255]);
    // `expect` starts a scene at a later event explicitly.
    let mut late = SlotScene::new([0.0; 3]);
    late.expect(2);
    assert_eq!(
        late.apply(&device, &queue, &two).unwrap(),
        Applied::Shown(vec![2])
    );
    assert_eq!(late.points(), 1);
}

#[test]
fn a_missing_event_is_recovered_from_a_folded_state() {
    let Some((device, queue, _gpu)) = gpu() else {
        return;
    };
    let cam = looking_north();
    let mut hl = Headless::new(&device, [W, H]);
    let a = [1.0f32, 0.0, 0.0];
    let b = [-1.0f32, 0.0, 0.0];
    let c = [0.0f32, 0.0, 0.6];
    let (ax, ay) = at(&cam, a.map(f64::from));
    let (bx, by) = at(&cam, b.map(f64::from));
    let (cx, cy) = at(&cam, c.map(f64::from));
    let mut s = SlotScene::new([0.0; 3]);
    s.apply(
        &device,
        &queue,
        &msg(1, 1, &[(0, a, [255, 0, 0])], &[], &[]),
    )
    .unwrap();
    // Event 2 (move slot 0 to b) is lost; 3 and 4 are held, the screen stays at event 1.
    for m in [
        msg(3, 2, &[(1, c, [0, 255, 0])], &[], &[]),
        msg(4, 2, &[], &[(1, c, [0, 200, 0])], &[]),
    ] {
        assert_eq!(s.apply(&device, &queue, &m).unwrap(), Applied::Held);
    }
    assert_eq!((s.seq(), s.held()), (Some(1), 2));
    // Folded state through 2 (slot 0 at b), then 3 and 4 from the held queue.
    let state2 = msg(2, 1, &[(0, b, [255, 0, 0])], &[], &[]);
    assert_eq!(
        s.apply_state(&device, &queue, &state2).unwrap(),
        Applied::Shown(vec![2, 3, 4])
    );
    assert_eq!((s.seq(), s.held(), s.points()), (Some(4), 0, 2));
    let img = hl.capture_with_slots(&device, &queue, &cam, &[], &[&s]);
    assert_eq!(pixel(&img, ax, ay), [0, 0, 0, 255]);
    assert_eq!(pixel(&img, bx, by), [255, 0, 0, 255]);
    assert_eq!(pixel(&img, cx, cy), [0, 200, 0, 255]);
}

#[test]
fn a_message_too_large_for_the_device_is_refused_before_holding() {
    let Some((device, queue, _gpu)) = gpu() else {
        return;
    };
    let mut s = SlotScene::new([0.0; 3]);
    let p = [0.0f32; 3];
    s.apply(&device, &queue, &msg(1, 1, &[(0, p, [1; 3])], &[], &[]))
        .unwrap();
    // Seq 3 needs more slots than any buffer: refused at once, not held in front of seq 2.
    let huge = msg(3, u32::MAX - 1, &[], &[], &[]);
    assert!(matches!(
        s.apply(&device, &queue, &huge),
        Err(pointblitz_core::ApplyError::TooManySlots { .. })
    ));
    assert_eq!(s.held(), 0);
    let two = msg(2, 1, &[], &[(0, p, [2; 3])], &[]);
    assert_eq!(
        s.apply(&device, &queue, &two).unwrap(),
        Applied::Shown(vec![2])
    );
}

#[test]
fn growing_keeps_the_slots_already_on_the_gpu() {
    let Some((device, queue, _gpu)) = gpu() else {
        return;
    };
    let cam = looking_north();
    let mut hl = Headless::new(&device, [W, H]);
    let mut s = SlotScene::new([0.0; 3]);
    let a = [1.0f32, 0.0, 0.0];
    let b = [-1.0f32, 0.0, 0.0];
    s.apply(
        &device,
        &queue,
        &msg(1, 1, &[(0, a, [255, 0, 0])], &[], &[]),
    )
    .unwrap();
    let small = s.gpu_bytes();
    // Slot 5000 needs a larger buffer; only slot 5000 is written, slot 0 comes from the GPU copy.
    s.apply(
        &device,
        &queue,
        &msg(2, 5001, &[(5000, b, [0, 0, 255])], &[], &[]),
    )
    .unwrap();
    assert!(s.gpu_bytes() > small);
    assert_eq!(s.last_upload_bytes(), 16);
    let img = hl.capture_with_slots(&device, &queue, &cam, &[], &[&s]);
    let (ax, ay) = at(&cam, a.map(f64::from));
    let (bx, by) = at(&cam, b.map(f64::from));
    assert_eq!(pixel(&img, ax, ay), [255, 0, 0, 255]);
    assert_eq!(pixel(&img, bx, by), [0, 0, 255, 255]);
}

#[test]
fn slot_and_chunk_scenes_share_one_pass_and_depth() {
    let Some((device, queue, _gpu)) = gpu() else {
        return;
    };
    let cam = looking_north();
    let mut hl = Headless::new(&device, [W, H]);
    // Chunk scene: a far red point and a near one off to the side. Slot scene: a near green point
    // in front of the far red one.
    let mut chunks = Scene::new();
    let pt = |p: [f32; 3], c: [u8; 3]| Point {
        position: p,
        color: c,
    };
    let side = [1.5f32, 0.0, 0.0];
    chunks
        .insert(
            &device,
            &encode_chunk(
                0,
                0,
                FLAG_LAST_IN_GENERATION,
                &[pt([0.0, 5.0, 0.0], [255, 0, 0]), pt(side, [255, 255, 255])],
            ),
        )
        .unwrap();
    let mut s = SlotScene::new([0.0; 3]);
    s.apply(
        &device,
        &queue,
        &msg(1, 1, &[(0, [0.0, -5.0, 0.0], [0, 255, 0])], &[], &[]),
    )
    .unwrap();
    let img = hl.capture_with_slots(&device, &queue, &cam, &[&chunks], &[&s]);
    let (x, y) = at(&cam, [0.0, 0.0, 0.0]);
    assert_eq!(pixel(&img, x, y), [0, 255, 0, 255]);
    let (sx, sy) = at(&cam, side.map(f64::from));
    assert_eq!(pixel(&img, sx, sy), [255, 255, 255, 255]);
    // A slot point behind a chunk point is hidden by it, though slot scenes are drawn last.
    let mut far_slot = SlotScene::new([0.0; 3]);
    far_slot
        .apply(
            &device,
            &queue,
            &msg(1, 1, &[(0, [0.0, 8.0, 0.0], [0, 0, 255])], &[], &[]),
        )
        .unwrap();
    let img = hl.capture_with_slots(&device, &queue, &cam, &[&chunks], &[&far_slot]);
    assert_eq!(pixel(&img, x, y), [255, 0, 0, 255]);
}

#[test]
fn stream_origin_far_away_still_renders_at_the_right_pixel() {
    let Some((device, queue, _gpu)) = gpu() else {
        return;
    };
    let origin = [300_000.0, 4_000_000.0, 120.0];
    let rel = [0.3f32, 3.0, 0.1];
    let mut s = SlotScene::new(origin);
    s.apply(
        &device,
        &queue,
        &msg(1, 1, &[(0, rel, [255, 128, 0])], &[], &[]),
    )
    .unwrap();
    let world = [0, 1, 2].map(|a| origin[a] + f64::from(rel[a]));
    let cam = Camera::new(
        [origin[0], origin[1], origin[2]],
        [origin[0], origin[1] + 3.0, origin[2]],
        [0.0, 0.0, 1.0],
        50.0,
    );
    let img = Headless::new(&device, [W, H]).capture_with_slots(&device, &queue, &cam, &[], &[&s]);
    let (x, y) = at(&cam, world);
    assert_eq!(pixel(&img, x, y), [255, 128, 0, 255]);
    let (lo, hi) = s.bounds().unwrap();
    assert_eq!(lo, world);
    assert_eq!(hi, world);
}

/// Real data: every theater30 event applied in order; for each, the time from `apply` until the GPU
/// finished the first frame that draws it (method (a), decision 0052).
#[test]
fn theater30_replace_events_apply_in_one_frame() {
    let Ok(dir) = std::env::var("PB_THEATER30_STREAM") else {
        eprintln!("PB_THEATER30_STREAM not set — skipped");
        return;
    };
    let Some((device, queue, _gpu)) = gpu() else {
        return;
    };
    let mut stream = pointblitz_io::slots::dir::StreamDir::open(&dir).unwrap();
    let messages = stream.messages().unwrap();
    let mut hl = Headless::new(&device, [1920, 1080]);
    // The whole final cloud in view, from the south-west and above.
    let mut fold = pointblitz_io::slots::Fold::new();
    for (_, m) in &messages {
        fold.apply(m).unwrap();
    }
    let mut s = SlotScene::new([0.0; 3]);
    s.apply_state(&device, &queue, &fold.message()).unwrap();
    let cam = fit(s.bounds().unwrap());
    s.clear();
    for (e, m) in &messages {
        let t0 = std::time::Instant::now();
        let r = s.apply(&device, &queue, m).unwrap();
        let t1 = std::time::Instant::now();
        hl.render_with_slots(&device, &queue, &cam, &[], &[&s]);
        let t2 = std::time::Instant::now();
        assert_eq!(r, Applied::Shown(vec![e.seq]));
        assert_eq!(s.points(), e.points, "event {}", e.seq);
        println!(
            "seq {:2} {:8} move {:6}: apply {:6.2} ms (upload {:5.1} MB), frame with it done {:6.2} ms, total {:6.2} ms",
            e.seq,
            e.kind,
            e.n_move,
            (t1 - t0).as_secs_f64() * 1e3,
            s.last_upload_bytes() as f64 / 1e6,
            (t2 - t1).as_secs_f64() * 1e3,
            (t2 - t0).as_secs_f64() * 1e3
        );
    }
    assert_eq!(s.capacity(), stream.manifest().capacity);
    // For comparison: a synchronised frame of the last state without any change.
    let t = std::time::Instant::now();
    for _ in 0..10 {
        hl.render_with_slots(&device, &queue, &cam, &[], &[&s]);
    }
    println!(
        "last state, frame without changes: {:.2} ms",
        t.elapsed().as_secs_f64() * 1e2
    );
}

/// A camera that sees the box from the south-west and above.
fn fit((lo, hi): ([f64; 3], [f64; 3])) -> Camera {
    let c = [0, 1, 2].map(|a| 0.5 * (lo[a] + hi[a]));
    let r = 0.5 * (0..3).map(|a| (hi[a] - lo[a]).powi(2)).sum::<f64>().sqrt();
    let d = r / (25f64.to_radians()).sin();
    let dir = [-0.5, -0.7, 0.5f64];
    let n = dir.iter().map(|v| v * v).sum::<f64>().sqrt();
    let eye = [0, 1, 2].map(|a| c[a] + dir[a] / n * d);
    Camera::new(eye, c, [0.0, 0.0, 1.0], 50.0)
}

// ---- Upload method (b), scatter (decision 0052 §2) ---------------------------------------------

/// Small deterministic generator (xorshift64*).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    fn coord(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32 * 4.0 - 2.0
    }

    fn record(&mut self, id: u32) -> (u32, [f32; 3], [u8; 3]) {
        (
            id,
            [self.coord(), self.coord(), self.coord()],
            [self.next() as u8, self.next() as u8, self.next() as u8],
        )
    }
}

/// A synthetic stream like cumulus3d's: new ids are handed out upwards, moves and removes pick
/// distinct live ids (an id is not moved and removed in the same event, a freed id is not reused in
/// it). Every fourth event adds 3000 points, so the buffer grows several times.
fn synthetic_stream(seed: u64, events: u32) -> Vec<Vec<u8>> {
    let mut rng = Rng(seed);
    let mut live: Vec<u32> = Vec::new();
    let mut next_id = 0u32;
    let mut out = Vec::new();
    for seq in 1..=events {
        let n_add = if seq % 4 == 2 { 3000 } else { rng.below(400) };
        let adds: Vec<_> = (0..n_add as u32).map(|k| rng.record(next_id + k)).collect();
        let n_move = rng.below(live.len() / 2 + 1).min(live.len());
        let n_remove = rng.below(live.len() / 4 + 1).min(live.len() - n_move);
        // Partial shuffle: the first n_move live ids move, the next n_remove go.
        for k in 0..n_move + n_remove {
            let j = k + rng.below(live.len() - k);
            live.swap(k, j);
        }
        let moves: Vec<_> = live[..n_move].iter().map(|&id| rng.record(id)).collect();
        let removes: Vec<u32> = live.drain(n_move..n_move + n_remove).collect();
        live.extend(adds.iter().map(|a| a.0));
        next_id += n_add as u32;
        out.push(msg(seq, next_id, &adds, &moves, &removes));
    }
    out
}

/// A mirror scene and a scatter scene, or None (skipped) when the device cannot scatter.
fn both(device: &wgpu::Device) -> Option<[SlotScene; 2]> {
    if !UploadMethod::Scatter.supported(device) {
        eprintln!("device without compute shaders / storage buffers — skipped");
        return None;
    }
    Some([
        SlotScene::new([0.0; 3]).with_upload(UploadMethod::Mirror),
        SlotScene::new([0.0; 3]).with_upload(UploadMethod::Scatter),
    ])
}

/// The two scenes hold the same bytes on the GPU and say the same things about them.
fn assert_same(device: &wgpu::Device, queue: &wgpu::Queue, s: &[SlotScene; 2], what: &str) {
    let [a, b] = s;
    assert_eq!(a.upload(), UploadMethod::Mirror);
    assert_eq!(b.upload(), UploadMethod::Scatter);
    assert_eq!(
        (a.capacity(), a.points(), a.seq(), a.held(), a.gpu_bytes()),
        (b.capacity(), b.points(), b.seq(), b.held(), b.gpu_bytes()),
        "{what}"
    );
    let (sa, sb) = (a.read_slots(device, queue), b.read_slots(device, queue));
    assert_eq!(sa.len(), a.capacity() as usize * 16);
    if let Some(i) = (0..sa.len()).find(|&i| sa[i] != sb[i]) {
        let at = i / 16 * 16;
        panic!(
            "{what}: slot {} differs: mirror {:?}, scatter {:?}",
            i / 16,
            &sa[at..at + 16],
            &sb[at..at + 16]
        );
    }
}

fn same_capture(
    hl: &mut Headless,
    gpu: (&wgpu::Device, &wgpu::Queue),
    cam: &Camera,
    s: &[SlotScene; 2],
) -> bool {
    let (device, queue) = gpu;
    let a = hl.capture_with_slots(device, queue, cam, &[], &[&s[0]]);
    let b = hl.capture_with_slots(device, queue, cam, &[], &[&s[1]]);
    a == b
}

#[test]
fn scatter_writes_the_same_slots_as_the_mirror() {
    let Some((device, queue, _gpu)) = gpu() else {
        return;
    };
    let cam = Camera::new([0.0, -8.0, 1.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 50.0);
    let mut hl = Headless::new(&device, [W, H]);
    let stream = synthetic_stream(7, 16);
    // Device limits, then bindings of 4 KiB (256 slots, about 200 records) and 2 workgroups per
    // dimension, so packs are cut into batches, slots into windows and dispatches go 2-D.
    for cap in [None, Some((4096u64, 2u32))] {
        let Some(mut s) = both(&device) else { return };
        if let Some((bytes, groups)) = cap {
            s[1] = SlotScene::new([0.0; 3])
                .with_upload(UploadMethod::Scatter)
                .with_scatter_limits(bytes, groups);
        }
        for (k, m) in stream.iter().enumerate() {
            let what = format!("{cap:?}, event {}", k + 1);
            for scene in &mut s {
                let r = scene.apply(&device, &queue, m).unwrap();
                assert_eq!(r, Applied::Shown(vec![k as u32 + 1]), "{what}");
            }
            assert_same(&device, &queue, &s, &what);
            assert!(
                same_capture(&mut hl, (&device, &queue), &cam, &s),
                "{what}: captures differ"
            );
        }
        // Lost event 11, 12 and 13 held: the folded state through 11 replaces the scene, 12 and 13
        // follow in the same call.
        let mut fold = pointblitz_io::slots::Fold::new();
        for m in &stream[..11] {
            fold.apply(m).unwrap();
        }
        let state = fold.message();
        for scene in &mut s {
            scene.clear();
            assert_eq!(
                scene.apply(&device, &queue, &stream[11]).unwrap(),
                Applied::Held
            );
            assert_eq!(
                scene.apply(&device, &queue, &stream[12]).unwrap(),
                Applied::Held
            );
            assert_eq!(
                scene.apply_state(&device, &queue, &state).unwrap(),
                Applied::Shown(vec![11, 12, 13])
            );
        }
        assert_same(&device, &queue, &s, &format!("{cap:?}, recovered"));
        assert!(
            same_capture(&mut hl, (&device, &queue), &cam, &s),
            "{cap:?}: captures differ after recovery"
        );
    }
}

#[test]
fn scatter_keeps_ordering_growth_and_bounds() {
    let Some((device, queue, _gpu)) = gpu() else {
        return;
    };
    let Some(mut s) = both(&device) else { return };
    let a = [1.0f32, 0.0, 0.0];
    let b = [-1.0f32, 0.0, 0.5];
    let one = msg(1, 2, &[(0, a, [255, 0, 0]), (1, b, [0, 255, 0])], &[], &[]);
    // Slot 5000 forces a grow; slot 0 is moved, slot 1 removed in the same event.
    let two = msg(
        2,
        5001,
        &[(5000, b, [0, 0, 255])],
        &[(0, b, [9, 9, 9])],
        &[1],
    );
    let empty = msg(3, 5001, &[], &[], &[]);
    for scene in &mut s {
        assert_eq!(scene.apply(&device, &queue, &two).unwrap(), Applied::Held);
        assert_eq!(
            scene.apply(&device, &queue, &one).unwrap(),
            Applied::Shown(vec![1, 2])
        );
        assert_eq!(
            scene.apply(&device, &queue, &empty).unwrap(),
            Applied::Shown(vec![3])
        );
        assert_eq!(scene.apply(&device, &queue, &one).unwrap(), Applied::Stale);
        assert_eq!(scene.points(), 2);
    }
    assert_same(&device, &queue, &s, "after grow");
    assert_eq!(
        s[1].last_upload_bytes(),
        0,
        "an empty message writes nothing"
    );
    // Exact bounds with the mirror (slots 0 and 5000, both at b); with scatter the box of
    // everything written (slot 0 was at a before it moved).
    let b64 = b.map(f64::from);
    assert_eq!(s[0].bounds(), Some((b64, b64)));
    assert_eq!(s[1].bounds(), Some(([-1.0, 0.0, 0.0], [1.0, 0.0, 0.5])));
    // A removed far point stays in the scatter box but not in the mirror's.
    let far = msg(4, 5002, &[(5001, [50.0, 0.0, 0.0], [1; 3])], &[], &[]);
    let gone = msg(5, 5002, &[], &[], &[5001]);
    for scene in &mut s {
        scene.apply(&device, &queue, &far).unwrap();
        scene.apply(&device, &queue, &gone).unwrap();
    }
    assert_same(&device, &queue, &s, "after remove");
    assert_eq!(s[0].bounds(), Some((b64, b64)));
    assert_eq!(s[1].bounds().unwrap().1[0], 50.0);
    // clear keeps the method.
    s[1].clear();
    assert_eq!(s[1].upload(), UploadMethod::Scatter);
    assert_eq!(s[1].bounds(), None);
}

/// Real data: every theater30 event applied with both methods; after each the slot buffers are
/// byte for byte the same and so are captures from two viewpoints. Then the folded state of the
/// whole stream through `apply_state`.
#[test]
fn theater30_scatter_matches_mirror() {
    let Ok(dir) = std::env::var("PB_THEATER30_STREAM") else {
        eprintln!("PB_THEATER30_STREAM not set — skipped");
        return;
    };
    let Some((device, queue, _gpu)) = gpu() else {
        return;
    };
    let Some(mut s) = both(&device) else { return };
    let mut stream = pointblitz_io::slots::dir::StreamDir::open(&dir).unwrap();
    let messages = stream.messages().unwrap();
    let mut fold = pointblitz_io::slots::Fold::new();
    for (_, m) in &messages {
        fold.apply(m).unwrap();
    }
    let state = fold.message();
    s[0].apply_state(&device, &queue, &state).unwrap();
    let (lo, hi) = s[0].bounds().unwrap();
    let wide = fit((lo, hi));
    let c = [0, 1, 2].map(|a| 0.5 * (lo[a] + hi[a]));
    let close = Camera::new(
        [c[0] - 20.0, c[1] - 30.0, c[2] + 15.0],
        c,
        [0.0, 0.0, 1.0],
        50.0,
    );
    let mut hl = Headless::new(&device, [1280, 720]);
    s[0].clear();
    let compare = |hl: &mut Headless, s: &[SlotScene; 2], what: &str| {
        assert_same(&device, &queue, s, what);
        for (name, cam) in [("wide", &wide), ("close", &close)] {
            assert!(
                same_capture(hl, (&device, &queue), cam, s),
                "{what}, {name}: captures differ"
            );
        }
    };
    for (e, m) in &messages {
        for scene in &mut s {
            assert_eq!(
                scene.apply(&device, &queue, m).unwrap(),
                Applied::Shown(vec![e.seq])
            );
            assert_eq!(scene.points(), e.points, "event {}", e.seq);
        }
        compare(&mut hl, &s, &format!("event {}", e.seq));
        println!(
            "seq {:2} {:8}: same slots ({} points), mirror wrote {:.1} MB, scatter {:.1} MB",
            e.seq,
            e.kind,
            e.points,
            s[0].last_upload_bytes() as f64 / 1e6,
            s[1].last_upload_bytes() as f64 / 1e6
        );
    }
    assert_eq!(s[1].capacity(), stream.manifest().capacity);
    let one_by_one = s[1].read_slots(&device, &queue);
    for scene in &mut s {
        scene.apply_state(&device, &queue, &state).unwrap();
    }
    compare(&mut hl, &s, "folded state");
    // Hidden slots keep their last bytes event by event but are zero in the folded state, so
    // compare the shown slots only: the same slots with the same bytes.
    let folded = s[1].read_slots(&device, &queue);
    let shown = |b: &[u8]| -> Vec<[u8; 16]> {
        b.as_chunks::<16>()
            .0
            .iter()
            .map(|s| if s[15] & 1 != 0 { *s } else { [0; 16] })
            .collect()
    };
    assert!(
        shown(&one_by_one) == shown(&folded),
        "folded state and events differ"
    );
}
