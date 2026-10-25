// Rain and snow (`particle.fsh` through the WEATHER pipeline): texture × white alpha × lightmap,
// fogged. Follows globals.wgsl.

@group(0) @binding(1) var streaks: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha: f32,
    @location(2) light: vec2<f32>,
    @location(3) rel: vec3<f32>,
};

@vertex
fn vs_main(@location(0) position: vec3<f32>, @location(1) uv: vec2<f32>, @location(2) alpha: f32, @location(3) light: vec2<f32>) -> VsOut {
    return VsOut(g.view_proj * vec4<f32>(position, 1.0), uv, alpha, light, position);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(streaks, samp, in.uv);
    let a = texel.a * in.alpha;
    if a < 0.1 {
        discard;
    }
    return vec4<f32>(fogged(texel.rgb * curve(in.light), fog_dist(in.rel)), a);
}
