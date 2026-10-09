// Sign text: glyph quads of the UI atlas on the signs' boards. Follows globals.wgsl.

@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) rel: vec3<f32>,
};

@vertex
fn vs_main(@location(0) position: vec3<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>, @location(3) light: vec2<f32>) -> VsOut {
    var out: VsOut;
    out.clip = g.view_proj * vec4(position, 1.0);
    out.uv = uv / vec2<f32>(textureDimensions(atlas));
    // The games' sRGB text colours, lit by the sign's cell and not by where the board faces.
    let tint = select(pow((color.rgb + 0.055) / 1.055, vec3(2.4)), color.rgb / 12.92, color.rgb <= vec3(0.04045));
    out.color = tint * curve(light);
    out.rel = position;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(atlas, atlas_sampler, in.uv);
    if texel.a < 0.1 {
        discard;
    }
    return vec4(fogged(texel.rgb * in.color, fog_dist(in.rel)), 1.0);
}
