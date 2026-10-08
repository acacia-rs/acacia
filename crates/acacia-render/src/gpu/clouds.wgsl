// Fancy clouds: boxes of crate::clouds, in cells around the camera's cell. Follows globals.wgsl.

struct Clouds {
    // xy: the camera's offset from the mesh's centre cell, in cells; z: the layer's foot above
    // the camera in blocks; w: the fade radius in blocks
    at: vec4<f32>,
    // rgb: the weather's tint (Java's cloud colour under rain and thunder)
    tint: vec4<f32>,
};

@group(0) @binding(1) var<uniform> clouds: Clouds;

/// Blocks per cell.
const CELL: f32 = 12.0;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) rel: vec3<f32>,
    @location(1) shade: f32,
};

@vertex
fn vs_main(@location(0) position: vec3<f32>, @location(1) shade: f32) -> VsOut {
    let xz = (position.xz - clouds.at.xy) * CELL;
    let rel = vec3<f32>(xz.x, position.y + clouds.at.z, xz.y);
    var out: VsOut;
    out.clip = g.view_proj * vec4<f32>(rel, 1.0);
    out.rel = rel;
    out.shade = shade;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Fades out towards the edge of the drawn range, as fog would.
    let fade = 1.0 - smoothstep(0.6, 1.0, length(in.rel.xz) / clouds.at.w);
    return vec4<f32>(curve(vec2<f32>(0.0, 15.0)) * clouds.tint.rgb * in.shade, 0.8 * fade);
}
