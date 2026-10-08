// Sprite particles: camera-facing quads cut from the particle sheet. Follows globals.wgsl.

struct View {
    // xyz: the camera's right and up in world space
    right: vec4<f32>,
    up: vec4<f32>,
};

@group(0) @binding(1) var<uniform> view: View;
@group(0) @binding(2) var sheet: texture_2d<f32>;
@group(0) @binding(3) var sheet_sampler: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) brightness: f32,
    @location(3) @interpolate(flat) blend: u32,
    @location(4) rel: vec3<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) index: u32,
    @location(0) centre: vec3<f32>,
    @location(1) size: f32,
    @location(2) uv: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) light: vec2<f32>,
    @location(5) blend: u32,
) -> VsOut {
    var corners = array<vec2<f32>, 6>(vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0));
    let c = corners[index];
    let world = centre + (view.right.xyz * c.x + view.up.xyz * c.y) * size;
    var out: VsOut;
    out.clip = g.view_proj * vec4<f32>(world, 1.0);
    out.uv = mix(uv.xy, uv.zw, vec2(c.x, -c.y) * 0.5 + 0.5);
    // Tints are the games' sRGB vertex colours.
    out.color = vec4(pow(color.rgb, vec3(2.2)), color.a);
    out.brightness = curve(light);
    out.blend = blend;
    out.rel = world;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(sheet, sheet_sampler, in.uv) * in.color;
    let rgb = fogged(texel.rgb * in.brightness, fog_dist(in.rel));
    // 0: cut out at Java's 0.1 and opaque, else blended by alpha (premultiplied).
    if in.blend == 0u {
        if texel.a < 0.1 {
            discard;
        }
        return vec4(rgb, 1.0);
    }
    if texel.a < 0.004 {
        discard;
    }
    return vec4(rgb * texel.a, texel.a);
}
