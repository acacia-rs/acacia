// Entity models: baked rest-pose vertices placed by one matrix per bone. Follows globals.wgsl.
// Instance layout: src/gpu/entities.rs.

struct Instance {
    // x: block light, y: sky light, z: the colour overlay (entity::java::overlay::keep),
    // w: the glint texture (0 none, 1 the item's, 2 the armour's)
    light: vec4<f32>,
    // rgb: tint, a: 1 when the texture's alpha is a tint mask
    tint: vec4<f32>,
    // Bit per bone index
    hidden: vec4<u32>,
    // x: the instance's first matrix in `bones`
    bones: vec4<u32>,
    // xy: the glint's scale over the UVs, zw: how far it (or a swirl layer's texture) has slid
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
    @location(1) light: vec3<f32>,
    @location(2) dist: vec2<f32>,
    @location(3) @interpolate(flat) tint: vec4<f32>,
    @location(4) @interpolate(flat) hurt: f32,
    @location(5) glint_uv: vec2<f32>,
    @location(6) @interpolate(flat) glint: f32,
    @location(7) @interpolate(flat) slide: vec2<f32>,
};

// Java's glint strength, and its texture matrix's turn (rotateZ(π / 18)): see src/glint.rs.
const GLINT_STRENGTH: f32 = 0.75;
const GLINT_TURN: vec2<f32> = vec2(0.17364818, 0.98480775);

// Lighting's LEVEL pair (DIFFUSE_LIGHT_0 and _1, normalized), in world space as Java's entity
// normals are. TODO: the Nether's pair (0.2, -1, 0.7 for the second) once the globals say which.
const LIGHT_0: vec3<f32> = vec3(0.16169041, 0.80845207, -0.56591645);
const LIGHT_1: vec3<f32> = vec3(-0.16169041, 0.80845207, 0.56591645);

// Lighting's ENTITY_IN_UI pair (INVENTORY_DIFFUSE_LIGHT_0 and _1, normalized), in a picture's
// space (window pixels, y down): what g.light.y selects.
const UI_LIGHT_0: vec3<f32> = vec3(0.14002801, -0.70014004, 0.70014004);
const UI_LIGHT_1: vec3<f32> = vec3(-0.19611614, -0.98058068, 0.0);

// Java's minecraft_mix_light (entity.vsh); the Bedrock look keeps the terrain's directional shades.
fn diffuse(n: vec3<f32>) -> f32 {
    let in_ui = g.light.y != 0.0;
    if g.light.x == 0.0 {
        // A picture's y points down.
        let up = select(n.y, -n.y, in_ui);
        return n.x * n.x * 0.6 + n.z * n.z * 0.8 + up * up * select(0.5, 1.0, up > 0.0);
    }
    let l0 = select(LIGHT_0, UI_LIGHT_0, in_ui);
    let l1 = select(LIGHT_1, UI_LIGHT_1, in_ui);
    return min(1.0, (max(0.0, dot(l0, n)) + max(0.0, dot(l1, n))) * 0.6 + 0.4);
}

@vertex
fn vs_main(in: VsIn, @builtin(instance_index) index: u32) -> VsOut {
    let instance = instances[index];
    let bone = in.bone;
    let m = bones[instance.bones.x + bone];
    let rel = (m * vec4(in.position, 1.0)).xyz;
    // PoseStack's normal matrix: the inverse transpose (it differs from `m` on a part scaled
    // unevenly), as the cofactor matrix signed by the determinant.
    let c0 = m[0].xyz;
    let c1 = m[1].xyz;
    let c2 = m[2].xyz;
    let flip = select(1.0, -1.0, dot(c0, cross(c1, c2)) < 0.0);
    let n = normalize(mat3x3(cross(c1, c2), cross(c2, c0), cross(c0, c1)) * in.normal * flip);
    var out: VsOut;
    out.clip = g.view_proj * vec4(rel, 1.0);
    if ((instance.hidden[bone / 32u] >> (bone % 32u)) & 1u) == 1u {
        // Outside the clip volume: a hidden bone's triangles vanish whole.
        out.clip = vec4(2.0, 2.0, 2.0, 1.0);
    }
    out.uv = in.uv;
    out.light = diffuse(n) * select(vec3(curve(instance.light.xy)), lightmap(instance.light.xy), g.light.x != 0.0);
    out.dist = fog_dist(rel);
    out.tint = instance.tint;
    out.hurt = instance.light.z;
    let scaled = in.uv * instance.glint.xy;
    out.glint_uv = vec2(scaled.x * GLINT_TURN.y - scaled.y * GLINT_TURN.x, scaled.x * GLINT_TURN.x + scaled.y * GLINT_TURN.y) + instance.glint.zw;
    out.glint = instance.light.w;
    out.slide = instance.glint.zw;
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
    // Java's OverlayTexture before the light: |hurt| of the colour stays, the rest goes red when
    // negative, white otherwise (entity::java::overlay::keep).
    let towards = select(vec3(1.0), vec3(1.0, 0.0, 0.0), in.hurt < 0.0);
    rgb = mix(towards, rgb, abs(in.hurt));
    return fogged(glinted(lit(rgb, in.light), in), in.dist);
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

// Java's `eyes` (EMISSIVE, NO_OVERLAY, NO_CARDINAL_LIGHTING): the texel as it is, fogged, over
// what is behind by its alpha.
@fragment
fn fs_eyes(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(skin, skin_sampler, in.uv);
    return vec4(fogged(texel.rgb, in.dist), texel.a);
}

// A charged creeper's aura (Java's rendertype_energy_swirl with EnergySwirlLayer's grey): the
// texture slides over the shell and is added to the frame at half strength, unlit, fading in fog.
// Java adds sRGB values; this adds linear ones, so it is fainter over a bright background.
@fragment
fn fs_swirl(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(skin, skin_sampler, fract(in.uv + in.slide));
    if texel.a < 0.1 {
        discard;
    }
    let clear = fogged(vec3(1.0), in.dist) - fogged(vec3(0.0), in.dist);
    return vec4(to_linear(to_srgb(texel.rgb) * 0.5) * clear, 0.0);
}
