// The UI over the world: one instance per quad, in window pixels, sampling the UI atlas.

struct Screen {
    // xy: window size in pixels, zw: atlas size in texels
    size: vec4<f32>,
    // xy: how far the glint texture has slid, z: its scale over an icon
    glint: vec4<f32>,
};

@group(0) @binding(0) var<uniform> screen: Screen;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
// The item glint (4 is the armour's, unused here): a plain texture, its texels sRGB values.
@group(0) @binding(3) var glint_item: texture_2d<f32>;
@group(0) @binding(5) var glint_sampler: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) glint_uv: vec2<f32>,
    @location(3) @interpolate(flat) glint: f32,
};

// As entity.wgsl's: see src/glint.rs.
const GLINT_STRENGTH: f32 = 0.75;
const GLINT_TURN: vec2<f32> = vec2(0.17364818, 0.98480775);

@vertex
fn vs_main(@builtin(vertex_index) index: u32, @location(0) rect: vec4<f32>, @location(1) uv: vec4<f32>, @location(2) color: vec4<f32>, @location(3) glint: f32) -> VsOut {
    // Two triangles: corners 0 1 2, 0 2 3 counter-clockwise from the top left.
    var corners = array<vec2<f32>, 6>(vec2(0.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 1.0), vec2(0.0, 0.0), vec2(1.0, 1.0), vec2(1.0, 0.0));
    let c = corners[index];
    let pixel = mix(rect.xy, rect.zw, c);
    var out: VsOut;
    out.clip = vec4<f32>(pixel.x / screen.size.x * 2.0 - 1.0, 1.0 - pixel.y / screen.size.y * 2.0, 0.0, 1.0);
    out.uv = mix(uv.xy, uv.zw, c) / screen.size.zw;
    out.color = color;
    let scaled = c * screen.glint.z;
    out.glint_uv = vec2(scaled.x * GLINT_TURN.y - scaled.y * GLINT_TURN.x, scaled.x * GLINT_TURN.x + scaled.y * GLINT_TURN.y) + screen.glint.xy;
    out.glint = glint;
    return out;
}

// The atlas is an sRGB texture, decoded on sampling; the target is a plain view, so encode back.
fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3(0.0031308));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let sampled = textureSample(atlas, samp, in.uv);
    let texel = vec4(to_srgb(sampled.rgb), sampled.a) * in.color;
    if texel.a <= 0.0 {
        discard;
    }
    // Java's item.fsh on an icon: the glint texel times its strength, squared, added.
    if in.glint > 0.5 && texel.a >= 0.1 {
        let glint = GLINT_STRENGTH * textureSampleLevel(glint_item, glint_sampler, in.glint_uv, 0.0).rgb;
        return vec4(min(texel.rgb + glint * glint, vec3(1.0)), texel.a);
    }
    return texel;
}
