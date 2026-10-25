// Clouds (`clouds.vsh`/`clouds.fsh`): crate::clouds faces, coloured by their side times the cloud
// colour and faded out by the clouds' own spherical fog. Follows globals.wgsl.

struct Clouds {
    // `CloudColor`, sRGB
    color: vec4<f32>,
    // xyz: `CloudOffset`, w: `FogCloudsEnd`
    offset: vec4<f32>,
};

@group(0) @binding(1) var<uniform> clouds: Clouds;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) distance: f32,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(@location(0) position: vec3<f32>, @location(1) shade: f32) -> VsOut {
    let pos = position + clouds.offset.xyz;
    return VsOut(g.view_proj * vec4<f32>(pos, 1.0), length(pos), vec4<f32>(vec3(shade), 1.0) * clouds.color);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // linear_fog_value(distance, 0, FogCloudsEnd)
    let fog = select(clamp(in.distance / clouds.offset.w, 0.0, 1.0), 0.0, in.distance <= 0.0);
    return vec4<f32>(srgb_to_linear(in.color.rgb), in.color.a * (1.0 - fog));
}
