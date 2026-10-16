// Slot scenes, upload method (b) (decision 0052 §2): a message's v4 pack is uploaded as it is and
// these kernels scatter its records into the slot buffer (16 B per slot: f32 x y z, u8 r g b,
// u8 flags with bit 0 = shown — the same bytes the CPU mirror of method (a) writes).
// Records are 4-byte aligned, so the pack is read as u32 words (little endian, like the record):
//   add / move  5 words: id | x | y | z | r g b zone (r in the low byte)
//   remove      1 word:  id
// One dispatch covers one section piece (`word` = its first word in the bound pack range) and one
// window of slots (`slot_base` … + `slot_count`, bound at that slot); records of other windows are
// skipped. Dispatches of a pass run in order, so adds, then moves, then removes, as in the mirror.

struct Params {
    word: u32,
    count: u32,
    slot_base: u32,
    slot_count: u32,
};

@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var<storage, read> pack: array<u32>;
@group(0) @binding(2) var<storage, read_write> slots: array<u32>;

// Flags byte bit 0, in the slot's fourth word.
const SHOWN: u32 = 0x01000000u;

// Record index of this invocation: workgroups may be laid out in two dimensions (at most 65535 per
// dimension).
fn record(g: vec3<u32>, n: vec3<u32>) -> u32 {
    return g.x + g.y * n.x * 64u;
}

// Word of `id`'s slot in the bound window, or none when the slot is in another window.
fn slot_word(id: u32) -> i32 {
    if id < p.slot_base || id - p.slot_base >= p.slot_count {
        return -1;
    }
    return i32((id - p.slot_base) * 4u);
}

@compute @workgroup_size(64)
fn put(@builtin(global_invocation_id) g: vec3<u32>, @builtin(num_workgroups) n: vec3<u32>) {
    let i = record(g, n);
    if i >= p.count {
        return;
    }
    let r = p.word + i * 5u;
    let w = slot_word(pack[r]);
    if w < 0 {
        return;
    }
    let s = u32(w);
    slots[s] = pack[r + 1u];
    slots[s + 1u] = pack[r + 2u];
    slots[s + 2u] = pack[r + 3u];
    // RGB from the record, the zone byte dropped, the slot's other flag bits kept, shown set.
    slots[s + 3u] = (slots[s + 3u] & 0xff000000u) | (pack[r + 4u] & 0x00ffffffu) | SHOWN;
}

@compute @workgroup_size(64)
fn hide(@builtin(global_invocation_id) g: vec3<u32>, @builtin(num_workgroups) n: vec3<u32>) {
    let i = record(g, n);
    if i >= p.count {
        return;
    }
    let w = slot_word(pack[p.word + i]);
    if w < 0 {
        return;
    }
    let s = u32(w) + 3u;
    slots[s] = slots[s] & ~SHOWN;
}
