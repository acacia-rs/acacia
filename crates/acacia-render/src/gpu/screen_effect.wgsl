// Java's underwater overlay (ScreenEffectRenderer.renderWater) over the whole screen. Follows globals.wgsl.

struct Effect {
    // xy: half the screen's extent at Java's quad depth (0.5 ahead), zw: the texture's scroll
    view: vec4<f32>,
    // x: opacity, yz: block and sky light at the eye
    params: vec4<f32>,
};

@group(0) @binding(1) var<uniform> e: Effect;
@group(0) @binding(2) var overlay: texture_2d<f32>;
@group(0) @binding(3) var samp: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VsOut {
    let ndc = vec2<f32>(select(-1.0, 3.0, index == 1u), select(-1.0, 3.0, index == 2u));
    var out: VsOut;
    out.clip = vec4<f32>(ndc, 0.0, 1.0);
    out.ndc = ndc;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Java's quad spans -1..1 at that depth with uv 4..0 across and down; the screen sees its middle.
    let uv = 2.0 * (1.0 - in.ndc * e.view.xy) + e.view.zw;
    let texel = textureSample(overlay, samp, uv);
    return vec4<f32>(texel.rgb * curve(e.params.yz), texel.a * e.params.x);
}
