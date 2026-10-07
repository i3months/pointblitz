// Opaque round points of a fixed screen size (decisions 0006, 0007).
// One instance per point (the chunk body, stride 16), four vertices per instance (triangle strip).

struct Camera {
    // ENU → clip, with the camera at the origin (camera-relative, decision 0013).
    view_proj: mat4x4<f32>,
    viewport_px: vec2<f32>,
    point_px: f32,
    _pad: f32,
};

struct Chunk {
    // Chunk origin minus camera position (computed in f64 on the CPU).
    offset: vec4<f32>,
};

@group(0) @binding(0) var<uniform> cam: Camera;
@group(1) @binding(0) var<uniform> chunk: Chunk;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) corner: vec2<f32>,
};

@vertex
fn vs(
    @builtin(vertex_index) vi: u32,
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
) -> VsOut {
    // Strip order: (-1,-1) (1,-1) (-1,1) (1,1).
    let corner = vec2<f32>(f32(vi & 1u) * 2.0 - 1.0, f32(vi >> 1u) * 2.0 - 1.0);
    var clip = cam.view_proj * vec4<f32>(position + chunk.offset.xyz, 1.0);
    // Expand in clip space so the quad is point_px wide on screen regardless of depth.
    clip = vec4<f32>(clip.xy + corner * (cam.point_px / cam.viewport_px) * clip.w, clip.zw);
    var out: VsOut;
    out.clip = clip;
    out.color = color;
    out.corner = corner;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    if dot(in.corner, in.corner) > 1.0 {
        discard;
    }
    return in.color;
}
