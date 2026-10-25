// Lightning bolts: untextured quads added onto the frame, faded by the fog (`rendertype_lightning`).
// Follows globals.wgsl.

// crate::lightning::COLOR.
const COLOR: vec4<f32> = vec4<f32>(114.0, 114.0, 127.0, 76.0) / 255.0;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) rel: vec3<f32>,
};

@vertex
fn vs_main(@location(0) position: vec3<f32>) -> VsOut {
    return VsOut(g.view_proj * vec4<f32>(position, 1.0), position);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let c = COLOR * (1.0 - fog_value(fog_dist(in.rel)));
    return vec4<f32>(srgb_to_linear(c.rgb), c.a);
}
