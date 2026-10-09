// Entity shadows: quads on the ground, darkened by a soft dot. Follows globals.wgsl.

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha: f32,
};

@vertex
fn vs_main(@location(0) position: vec3<f32>, @location(1) uv: vec2<f32>, @location(2) alpha: f32) -> VsOut {
    var out: VsOut;
    out.clip = g.view_proj * vec4<f32>(position, 1.0);
    out.uv = uv;
    out.alpha = alpha;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // 0 at the blob's centre, 1 at its rim (the game's shadow image is a soft round dot).
    let from_centre = length(in.uv - vec2<f32>(0.5)) * 2.0;
    let blob = 1.0 - smoothstep(0.0, 1.0, from_centre);
    // Java blends in sRGB; this target is linear, where the same darkening needs more alpha.
    return vec4<f32>(0.0, 0.0, 0.0, 1.0 - pow(1.0 - in.alpha * blob, 2.2));
}
