// Cracks over a block being mined: its boxes textured with a destroy stage, multiplied onto the
// frame as Java's `crumbling` render type does (2 · src · dst). Follows globals.wgsl.

@group(0) @binding(1) var stages: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@location(0) position: vec3<f32>, @location(1) uv: vec2<f32>) -> VsOut {
    var out: VsOut;
    out.clip = g.view_proj * vec4<f32>(position, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(stages, samp, in.uv);
    if texel.a < 0.1 {
        discard;
    }
    return vec4<f32>(texel.rgb, 1.0);
}
