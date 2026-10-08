// Rain: camera-facing column quads with the streak texture, blended over the world. Follows globals.wgsl.

@group(0) @binding(1) var streaks: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

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
    let texel = textureSample(streaks, samp, in.uv);
    let a = texel.a * in.alpha;
    if a < 0.01 {
        discard;
    }
    // Lit as open sky is at this time of day.
    return vec4<f32>(texel.rgb * curve(vec2<f32>(0.0, 15.0)), a);
}
