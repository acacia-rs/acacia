// Java's post/box_blur.fsh over a whole-window triangle: see acacia_ui::blur.

struct Config {
    dir: vec2<f32>,
    // The input's size in texels.
    size: vec2<f32>,
    radius: f32,
};

@group(0) @binding(0) var<uniform> config: Config;
@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VsOut {
    let corner = vec2(f32((index << 1u) & 2u), f32(index & 2u));
    var out: VsOut;
    out.clip = vec4(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0);
    out.uv = corner;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let step = config.dir / config.size;
    let r = config.radius;
    var blurred = vec4(0.0);
    for (var a = -r + 0.5; a <= r; a += 2.0) {
        blurred += textureSampleLevel(source, samp, in.uv + step * a, 0.0);
    }
    blurred += textureSampleLevel(source, samp, in.uv + step * r, 0.0) / 2.0;
    return blurred / (r + 0.5);
}
