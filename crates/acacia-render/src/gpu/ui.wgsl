// The UI over the world: one instance per quad, in window pixels, sampling the UI atlas.

struct Screen {
    // xy: window size in pixels, zw: atlas size in texels
    size: vec4<f32>,
};

@group(0) @binding(0) var<uniform> screen: Screen;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32, @location(0) rect: vec4<f32>, @location(1) uv: vec4<f32>, @location(2) color: vec4<f32>) -> VsOut {
    // Two triangles: corners 0 1 2, 0 2 3 counter-clockwise from the top left.
    var corners = array<vec2<f32>, 6>(vec2(0.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 1.0), vec2(0.0, 0.0), vec2(1.0, 1.0), vec2(1.0, 0.0));
    let c = corners[index];
    let pixel = mix(rect.xy, rect.zw, c);
    var out: VsOut;
    out.clip = vec4<f32>(pixel.x / screen.size.x * 2.0 - 1.0, 1.0 - pixel.y / screen.size.y * 2.0, 0.0, 1.0);
    out.uv = mix(uv.xy, uv.zw, c) / screen.size.zw;
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(atlas, samp, in.uv) * in.color;
    if texel.a <= 0.0 {
        discard;
    }
    return texel;
}
