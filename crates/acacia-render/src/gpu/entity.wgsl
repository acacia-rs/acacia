// Entity models: baked rest-pose vertices placed by one matrix per bone. Follows globals.wgsl.
// Instance layout: src/gpu/entities.rs.

struct Instance {
    // x: block light, y: sky light, z: 1 while hurt or dying (Java's red overlay),
    // w: the glint texture (0 none, 1 the item's, 2 the armour's)
    light: vec4<f32>,
    // rgb: tint, a: 1 when the texture's alpha is a tint mask
    tint: vec4<f32>,
    // Bit per bone index
    hidden: vec4<u32>,
    // x: the instance's first matrix in `bones`
    bones: vec4<u32>,
    // xy: the glint's scale over the UVs, zw: how far it has slid
    glint: vec4<f32>,
};

@group(0) @binding(1) var<storage, read> instances: array<Instance>;
// Model space to camera-relative world space, per bone of each instance.
@group(0) @binding(2) var<storage, read> bones: array<mat4x4<f32>>;
// Plain (not sRGB) textures: the texels are added as the sRGB values they are.
@group(0) @binding(3) var glint_item: texture_2d<f32>;
@group(0) @binding(4) var glint_armor: texture_2d<f32>;
@group(0) @binding(5) var glint_sampler: sampler;
@group(1) @binding(0) var skin: texture_2d<f32>;
@group(1) @binding(1) var skin_sampler: sampler;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) bone: u32,
    @location(2) normal: vec3<f32>,
    @location(3) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) shade: f32,
    @location(2) dist: vec2<f32>,
    @location(3) @interpolate(flat) tint: vec4<f32>,
    @location(4) @interpolate(flat) hurt: f32,
    @location(5) glint_uv: vec2<f32>,
    @location(6) @interpolate(flat) glint: f32,
};

// Java's glint strength, and its texture matrix's turn (rotateZ(π / 18)): see src/glint.rs.
const GLINT_STRENGTH: f32 = 0.75;
const GLINT_TURN: vec2<f32> = vec2(0.17364818, 0.98480775);

@vertex
fn vs_main(in: VsIn, @builtin(instance_index) index: u32) -> VsOut {
    let instance = instances[index];
    let bone = in.bone;
    let m = bones[instance.bones.x + bone];
    let rel = (m * vec4(in.position, 1.0)).xyz;
    let n = normalize((m * vec4(in.normal, 0.0)).xyz);
    // The terrain's directional shades, blended by how much the normal faces each axis.
    let directional = n.x * n.x * 0.6 + n.z * n.z * 0.8 + n.y * n.y * select(0.5, 1.0, n.y > 0.0);
    var out: VsOut;
    out.clip = g.view_proj * vec4(rel, 1.0);
    if ((instance.hidden[bone / 32u] >> (bone % 32u)) & 1u) == 1u {
        // Outside the clip volume: a hidden bone's triangles vanish whole.
        out.clip = vec4(2.0, 2.0, 2.0, 1.0);
    }
    out.uv = in.uv;
    out.shade = directional * curve(instance.light.xy);
    out.dist = fog_dist(rel);
    out.tint = instance.tint;
    out.hurt = instance.light.z;
    let scaled = in.uv * instance.glint.xy;
    out.glint_uv = vec2(scaled.x * GLINT_TURN.y - scaled.y * GLINT_TURN.x, scaled.x * GLINT_TURN.x + scaled.y * GLINT_TURN.y) + instance.glint.zw;
    out.glint = instance.light.w;
    return out;
}

fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3(0.0031308));
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, c <= vec3(0.04045));
}

// Java's item.fsh and entity.fsh: the glint texel times its strength, squared, added to the lit
// colour before the fog. Java adds sRGB values; this target is linear, so convert around it.
fn glinted(lit: vec3<f32>, in: VsOut) -> vec3<f32> {
    if in.glint < 0.5 {
        return lit;
    }
    let item = textureSampleLevel(glint_item, glint_sampler, in.glint_uv, 0.0).rgb;
    let armor = textureSampleLevel(glint_armor, glint_sampler, in.glint_uv, 0.0).rgb;
    let texel = GLINT_STRENGTH * select(item, armor, in.glint > 1.5);
    return to_linear(min(to_srgb(lit) + texel * texel, vec3(1.0)));
}

// The lit, fogged colour of a texel.
fn shaded(in: VsOut, texel: vec4<f32>) -> vec3<f32> {
    var rgb = texel.rgb;
    if in.tint.a > 0.5 {
        rgb = mix(rgb * in.tint.rgb, rgb, texel.a);
    }
    // Java's hurt overlay: 30% red, before the light.
    rgb = mix(rgb, vec3(1.0, 0.0, 0.0), 0.3 * in.hurt);
    return fogged(glinted(rgb * in.shade, in), in.dist);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(skin, skin_sampler, in.uv);
    if in.tint.a <= 0.5 && texel.a < 0.1 {
        discard;
    }
    return vec4(shaded(in, texel), 1.0);
}

// Blended layers (a slime's shell): the texel's alpha is how much of it shows.
@fragment
fn fs_blend(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(skin, skin_sampler, in.uv);
    return vec4(shaded(in, texel), texel.a);
}
