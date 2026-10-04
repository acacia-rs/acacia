// Entity models: baked rest-pose vertices placed by two matrices per instance. Follows globals.wgsl.
// Instance layout: src/gpu/entities.rs. Vertex part bits: src/entity/bake.rs.

struct Instance {
    // Model space to camera-relative world space.
    body: mat4x4<f32>,
    // The same for vertices of the head part.
    head: mat4x4<f32>,
    // x: block light, y: sky light
    light: vec4<f32>,
    // rgb: tint, a: 1 when the texture's alpha is a tint mask
    tint: vec4<f32>,
    // Bit per bone index
    hidden: vec4<u32>,
};

@group(0) @binding(1) var<storage, read> instances: array<Instance>;
@group(1) @binding(0) var skin: texture_2d<f32>;
@group(1) @binding(1) var skin_sampler: sampler;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) part: u32,
    @location(2) normal: vec3<f32>,
    @location(3) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) shade: f32,
    @location(2) dist: f32,
    @location(3) @interpolate(flat) tint: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn, @builtin(instance_index) index: u32) -> VsOut {
    let instance = instances[index];
    var m = instance.body;
    if (in.part & 1u) == 1u {
        m = instance.head;
    }
    let rel = (m * vec4(in.position, 1.0)).xyz;
    let n = normalize((m * vec4(in.normal, 0.0)).xyz);
    // The terrain's directional shades, blended by how much the normal faces each axis.
    let directional = n.x * n.x * 0.6 + n.z * n.z * 0.8 + n.y * n.y * select(0.5, 1.0, n.y > 0.0);
    var out: VsOut;
    out.clip = g.view_proj * vec4(rel, 1.0);
    let bone = in.part >> 8u;
    if ((instance.hidden[bone / 32u] >> (bone % 32u)) & 1u) == 1u {
        // Outside the clip volume: a hidden bone's triangles vanish whole.
        out.clip = vec4(2.0, 2.0, 2.0, 1.0);
    }
    out.uv = in.uv;
    out.shade = directional * curve(instance.light.xy);
    out.dist = length(rel);
    out.tint = instance.tint;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(skin, skin_sampler, in.uv);
    var rgb = texel.rgb;
    if in.tint.a > 0.5 {
        rgb = mix(rgb * in.tint.rgb, rgb, texel.a);
    } else if texel.a < 0.1 {
        discard;
    }
    return vec4(fogged(rgb * in.shade, in.dist), 1.0);
}
