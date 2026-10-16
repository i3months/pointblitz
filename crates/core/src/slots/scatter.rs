//! Upload method (b) (decision 0052 §2): a message's pack is written to a storage buffer as it is
//! and `scatter.wgsl` scatters its records into the slot buffer.
//!
//! The pack is cut at record boundaries into batches of at most `max_storage_buffer_binding_size`
//! bytes (one in practice: theater30's largest pack is 14 MB, its folded state 57.6 MB, the WebGPU
//! default limit 128 MiB); each batch is one `write_buffer` and one submit, so a later batch can
//! reuse the buffer. Inside a batch every section piece (adds, moves, removes, in that order) is one
//! dispatch per window of the slot buffer, a window being at most the binding limit too. Submits
//! run in order, so a message is complete on the GPU before anything submitted after it (the next
//! frame) reads the slots.

use super::SLOT_BYTES;
use pointblitz_io::slots::{ADD_BYTES, HEADER_LEN, REMOVE_BYTES, SlotHeader};

/// Invocations per workgroup (`@workgroup_size` in scatter.wgsl).
const GROUP: u32 = 64;
/// Smallest pack buffer.
const MIN_PACK: u64 = 64 * 1024;

/// Which kernel a dispatch runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kernel {
    /// Add and move records: write the slot, show it.
    Put,
    /// Remove records: hide the slot.
    Hide,
}

/// Part of one section inside a batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Piece {
    pub kernel: Kernel,
    /// Binding range in the pack buffer (the batch is written at 0); `offset` is aligned.
    pub offset: u64,
    pub size: u64,
    /// First word of the piece's records in the binding.
    pub word: u32,
    pub count: u32,
}

/// Pack bytes `start..start + len`, written to the pack buffer at 0 and dispatched together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Batch {
    pub start: u64,
    pub len: u64,
    pub pieces: Vec<Piece>,
}

/// Cuts a message's pack into batches of at most `limit` bytes (`limit` a multiple of `align`, the
/// storage binding offset alignment, and larger than a record).
pub(super) fn plan(h: &SlotHeader, limit: u64, align: u64) -> Vec<Batch> {
    let sections = [
        (Kernel::Put, h.n_add, ADD_BYTES as u64),
        (Kernel::Put, h.n_move, ADD_BYTES as u64),
        (Kernel::Hide, h.n_remove, REMOVE_BYTES as u64),
    ];
    let mut batches = Vec::new();
    let mut batch = Batch {
        start: 0,
        len: 0,
        pieces: Vec::new(),
    };
    let mut at = 0u64;
    for (kernel, n, rec) in sections {
        let mut left = u64::from(n);
        while left > 0 {
            let room = (limit - batch.len) / rec;
            if room == 0 {
                let next = Batch {
                    start: at,
                    len: 0,
                    pieces: Vec::new(),
                };
                batches.push(std::mem::replace(&mut batch, next));
                continue;
            }
            let count = left.min(room);
            let start = at - batch.start;
            let end = start + count * rec;
            let offset = start / align * align;
            batch.pieces.push(Piece {
                kernel,
                offset,
                size: end - offset,
                word: ((start - offset) / 4) as u32,
                count: count as u32,
            });
            batch.len = end;
            at += count * rec;
            left -= count;
        }
    }
    if !batch.pieces.is_empty() {
        batches.push(batch);
    }
    batches
}

/// Windows `(first slot, slots)` over `capacity` slots, each at most `limit` bytes (`limit` a
/// multiple of `align`, which is a multiple of the slot size).
pub(super) fn slot_windows(capacity: u32, limit: u64) -> Vec<(u32, u32)> {
    let per = (limit / SLOT_BYTES as u64).min(u64::from(u32::MAX)) as u32;
    (0..capacity.div_ceil(per))
        .map(|k| (k * per, per.min(capacity - k * per)))
        .collect()
}

/// Workgroups for `count` records: `(x, y)` with x at most `max` (the per-dimension limit).
pub(super) fn groups(count: u32, max: u32) -> (u32, u32) {
    let g = count.div_ceil(GROUP);
    let x = g.min(max).max(1);
    (x, g.div_ceil(x))
}

pub(super) struct Scatter {
    layout: wgpu::BindGroupLayout,
    put: wgpu::ComputePipeline,
    hide: wgpu::ComputePipeline,
    pack: Option<wgpu::Buffer>,
    params: Option<wgpu::Buffer>,
    /// Bytes between dispatch parameters (uniform dynamic offset alignment).
    param_stride: u64,
    /// Storage binding offset alignment.
    align: u64,
    /// Largest storage binding, a multiple of `align`.
    limit: u64,
    /// Workgroups per dimension.
    max_groups: u32,
}

impl Scatter {
    /// True when the device has compute shaders and storage buffers (not WebGL2).
    pub fn supported(device: &wgpu::Device) -> bool {
        let l = device.limits();
        l.max_storage_buffers_per_shader_stage >= 2
            && l.max_compute_workgroups_per_dimension >= 1
            && l.max_compute_invocations_per_workgroup >= GROUP
            && l.max_compute_workgroup_size_x >= GROUP
            && l.max_storage_buffer_binding_size >= 4096
    }

    /// `cap`: lower binding size and workgroups-per-dimension limits than the device's (tests).
    pub fn new(device: &wgpu::Device, cap: Option<(u64, u32)>) -> Self {
        let l = device.limits();
        let align = u64::from(l.min_storage_buffer_offset_alignment).max(SLOT_BYTES as u64);
        let (mut binding, mut max_groups) = (
            l.max_storage_buffer_binding_size.min(l.max_buffer_size),
            l.max_compute_workgroups_per_dimension,
        );
        if let Some((b, g)) = cap {
            binding = binding.min(b);
            max_groups = max_groups.min(g);
        }
        let limit = binding / align * align;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pointblitz slot scatter"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../scatter.wgsl").into()),
        });
        let entry = |binding, ty, dynamic| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: dynamic,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("slot scatter"),
            entries: &[
                entry(0, wgpu::BufferBindingType::Uniform, true),
                entry(
                    1,
                    wgpu::BufferBindingType::Storage { read_only: true },
                    false,
                ),
                entry(
                    2,
                    wgpu::BufferBindingType::Storage { read_only: false },
                    false,
                ),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("slot scatter"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("slot scatter"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let ua = u64::from(l.min_uniform_buffer_offset_alignment);
        Self {
            put: pipeline("put"),
            hide: pipeline("hide"),
            layout,
            pack: None,
            params: None,
            param_stride: 16u64.div_ceil(ua) * ua,
            align,
            limit,
            max_groups,
        }
    }

    /// Scatters a validated message into the first `capacity` slots of `slots` (the slot buffer,
    /// with STORAGE usage). Returns the pack bytes written to the GPU.
    pub fn apply(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        slots: &wgpu::Buffer,
        capacity: u32,
        header: &SlotHeader,
        msg: &[u8],
    ) -> u64 {
        let pack = &msg[HEADER_LEN..];
        let windows = slot_windows(capacity, self.limit);
        let mut written = 0;
        for batch in plan(header, self.limit, self.align) {
            let bytes = &pack[batch.start as usize..(batch.start + batch.len) as usize];
            let pack_buf = reserve(
                device,
                &mut self.pack,
                "slot scatter pack",
                // Not `clamp`: a test limit can be below MIN_PACK.
                batch.len.next_power_of_two().max(MIN_PACK).min(self.limit),
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            );
            queue.write_buffer(&pack_buf, 0, bytes);
            written += batch.len;
            let dispatches: Vec<(Piece, (u32, u32))> = batch
                .pieces
                .iter()
                .flat_map(|p| windows.iter().map(move |w| (*p, *w)))
                .collect();
            let params_buf = reserve(
                device,
                &mut self.params,
                "slot scatter params",
                (dispatches.len() as u64 * self.param_stride).next_power_of_two(),
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            );
            let mut params = vec![0u8; dispatches.len() * self.param_stride as usize];
            for (k, (p, (base, n))) in dispatches.iter().enumerate() {
                let at = k * self.param_stride as usize;
                for (j, v) in [p.word, p.count, *base, *n].into_iter().enumerate() {
                    params[at + 4 * j..at + 4 * j + 4].copy_from_slice(&v.to_le_bytes());
                }
            }
            queue.write_buffer(&params_buf, 0, &params);
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("slot scatter"),
            });
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("slot scatter"),
                    timestamp_writes: None,
                });
                for (k, (p, (base, n))) in dispatches.iter().enumerate() {
                    let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("slot scatter"),
                        layout: &self.layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                    buffer: &params_buf,
                                    offset: 0,
                                    size: wgpu::BufferSize::new(16),
                                }),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                    buffer: &pack_buf,
                                    offset: p.offset,
                                    size: wgpu::BufferSize::new(p.size),
                                }),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                    buffer: slots,
                                    offset: u64::from(*base) * SLOT_BYTES as u64,
                                    size: wgpu::BufferSize::new(u64::from(*n) * SLOT_BYTES as u64),
                                }),
                            },
                        ],
                    });
                    pass.set_pipeline(match p.kernel {
                        Kernel::Put => &self.put,
                        Kernel::Hide => &self.hide,
                    });
                    pass.set_bind_group(0, &bg, &[(k as u64 * self.param_stride) as u32]);
                    let (x, y) = groups(p.count, self.max_groups);
                    pass.dispatch_workgroups(x, y, 1);
                }
            }
            queue.submit([enc.finish()]);
        }
        written
    }
}

/// The buffer in `slot`, replaced by a new one of `size` bytes when it is smaller.
fn reserve(
    device: &wgpu::Device,
    slot: &mut Option<wgpu::Buffer>,
    label: &str,
    size: u64,
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    match slot {
        Some(b) if b.size() >= size => b.clone(),
        _ => {
            let b = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            });
            *slot = Some(b.clone());
            b
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(n_add: u32, n_move: u32, n_remove: u32) -> SlotHeader {
        SlotHeader {
            seq: 1,
            n_add,
            n_move,
            n_remove,
            capacity: 1,
        }
    }

    /// Every record of every section is in exactly one piece, in order, inside its binding, and
    /// every binding is aligned and within the limit.
    fn check(h: &SlotHeader, limit: u64, align: u64) -> Vec<Batch> {
        let batches = plan(h, limit, align);
        let mut covered = [0u64; 3];
        let mut at = 0;
        for b in &batches {
            assert_eq!(b.start, at, "batches are contiguous");
            assert!(b.len <= limit);
            at += b.len;
            let mut end = 0;
            for p in &b.pieces {
                assert_eq!(p.offset % align, 0);
                assert!(p.size <= limit && p.offset + p.size <= b.len);
                let rec = if p.kernel == Kernel::Put { 20 } else { 4 };
                let first = p.offset + u64::from(p.word) * 4;
                assert_eq!(first, end, "pieces are contiguous");
                assert_eq!(first + u64::from(p.count) * rec, p.offset + p.size);
                end = p.offset + p.size;
                let section = match (p.kernel, b.start + first) {
                    (Kernel::Hide, _) => 2,
                    (_, s) if s < u64::from(h.n_add) * 20 => 0,
                    _ => 1,
                };
                covered[section] += u64::from(p.count);
            }
            assert_eq!(end, b.len);
        }
        assert_eq!(at, h.pack_len());
        assert_eq!(
            covered,
            [h.n_add, h.n_move, h.n_remove].map(u64::from),
            "{h:?}"
        );
        batches
    }

    #[test]
    fn a_pack_under_the_limit_is_one_batch_of_three_pieces() {
        let h = header(700_000, 540_000, 3);
        let b = check(&h, 128 << 20, 256);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].pieces.len(), 3);
        // The move section starts at byte 14,000,000 = 54687 × 256 + 128: bound from the aligned
        // offset below it, first record at word 32.
        assert_eq!(b[0].pieces[1].offset, 54_687 * 256);
        assert_eq!(b[0].pieces[1].word, 32);
        // Empty sections have no piece.
        assert_eq!(check(&header(0, 5, 0), 4096, 256)[0].pieces.len(), 1);
        assert!(check(&header(0, 0, 0), 4096, 256).is_empty());
    }

    #[test]
    fn a_pack_over_the_limit_is_cut_at_records() {
        for (a, m, r) in [(1000, 0, 0), (333, 777, 1234), (0, 0, 5000), (205, 0, 1)] {
            for (limit, align) in [(4096, 256), (1024, 256), (256, 256), (512, 64)] {
                let b = check(&header(a, m, r), limit, align);
                assert!(b.len() > 1 || header(a, m, r).pack_len() <= limit);
            }
        }
    }

    /// The kernels compile without a GPU (the GPU tests in tests/slots.rs run them).
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_kernels_are_valid_wgsl() {
        use wgpu::naga;
        let module = naga::front::wgsl::parse_str(include_str!("../scatter.wgsl"))
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(include_str!("../scatter.wgsl"))));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("valid");
        let entries: Vec<_> = module
            .entry_points
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(entries, ["put", "hide"]);
    }

    #[test]
    fn slot_windows_and_workgroups_respect_the_limits() {
        assert_eq!(slot_windows(2_881_929, 128 << 20), [(0, 2_881_929)]);
        assert_eq!(
            slot_windows(1000, 4096),
            [(0, 256), (256, 256), (512, 256), (768, 232)]
        );
        assert_eq!(slot_windows(0, 4096), []);
        assert_eq!(groups(700_000, 65_535), (10_938, 1));
        assert_eq!(groups(64 * 65_535 + 1, 65_535), (65_535, 2));
        assert_eq!(groups(1000, 4), (4, 4));
        assert_eq!(groups(1, 65_535), (1, 1));
    }
}
