// Flat clouds: one camera-centred quad at the cloud height sampling the cloud map. Follows globals.wgsl.

struct Clouds {
    // xy: the camera's position in cloud-map texels (wrapped), z: the layer's height above the
    // camera, w: half the quad's side in blocks
    at: vec4<f32>,
};

@group(0) @binding(1) var<uniform> clouds: Clouds;
@group(0) @binding(2) var map: texture_2d<f32>;
@group(0) @binding(3) var samp: sampler;

/// Blocks per cloud-map texel.
const TEXEL: f32 = 12.0;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) offset: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VsOut {
    var corners = array<vec2<f32>, 6>(vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0));
    let xz = corners[index] * clouds.at.w;
    var out: VsOut;
    out.clip = g.view_proj * vec4<f32>(xz.x, clouds.at.z, xz.y, 1.0);
    out.offset = xz;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(map));
    let texel = textureSample(map, samp, (clouds.at.xy + in.offset / TEXEL) / size);
    if texel.a < 0.5 {
        discard;
    }
    // Fades out towards the quad's edge, as fog would.
    let fade = 1.0 - smoothstep(0.5, 1.0, length(in.offset) / clouds.at.w);
    return vec4<f32>(vec3<f32>(curve(vec2<f32>(0.0, 15.0))), 0.8 * fade);
}
