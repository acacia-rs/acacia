// Particles: camera-facing quads cut from the block texture array. Follows globals.wgsl.

struct View {
    // xyz: the camera's right and up in world space
    right: vec4<f32>,
    up: vec4<f32>,
};

@group(0) @binding(1) var<uniform> view: View;
@group(0) @binding(2) var atlas: texture_2d_array<f32>;
@group(0) @binding(3) var atlas_sampler: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) layer: u32,
    @location(2) brightness: f32,
};

@vertex
fn vs_main(
    @builtin(vertex_index) index: u32,
    @location(0) centre: vec3<f32>,
    @location(1) size: f32,
    @location(2) piece: vec2<f32>,
    @location(3) layer: u32,
    @location(4) light: vec2<f32>,
) -> VsOut {
    var corners = array<vec2<f32>, 6>(vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0));
    let c = corners[index];
    let world = centre + (view.right.xyz * c.x + view.up.xyz * c.y) * size;
    var out: VsOut;
    out.clip = g.view_proj * vec4<f32>(world, 1.0);
    // A 4×4-texel piece of the 16×16 tile.
    out.uv = (piece + (vec2(c.x, -c.y) * 0.5 + 0.5) * 4.0) / 16.0;
    out.layer = layer;
    out.brightness = curve(light);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(atlas, atlas_sampler, in.uv, in.layer);
    if texel.a < 0.5 {
        discard;
    }
    return vec4<f32>(texel.rgb * in.brightness, 1.0);
}
