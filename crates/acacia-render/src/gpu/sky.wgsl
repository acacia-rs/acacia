// Java's sky (gpu/sky.rs), drawn before the terrain. Follows globals.wgsl.

struct Celestial {
    // The sun's, moon's, stars', sunrise fan's and dark disc's frames
    frames: array<mat4x4<f32>, 5>,
    // x: star brightness, y: moon phase (0..8), z: 1 - rain, w: where the sky is fully fogged
    params: vec4<f32>,
    // The sunrise colour: linear rgb and alpha
    glow: vec4<f32>,
    // rgb: the sky colour (linear); w: 1 when the dark disc shows
    zenith: vec4<f32>,
};

@group(0) @binding(1) var<uniform> sky: Celestial;
@group(0) @binding(2) var sun: texture_2d<f32>;
@group(0) @binding(3) var moon: texture_2d<f32>;
@group(0) @binding(4) var samp: sampler;
@group(0) @binding(5) var end_sky: texture_2d<f32>;

const SUN: u32 = 0u;
const MOON: u32 = 1u;
const STAR: u32 = 2u;
const GLOW: u32 = 3u;
const DOME: u32 = 4u;
const END: u32 = 5u;
const DARK: u32 = 6u;
const END_SHADE: f32 = 0.1568627;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) kind: u32,
    @location(2) model: vec3<f32>,
};

@vertex
fn vs_main(@location(0) position: vec3<f32>, @location(1) kind: u32, @location(2) uv: vec2<f32>) -> VsOut {
    var out: VsOut;
    var at = position;
    if kind <= GLOW {
        at = (sky.frames[kind] * vec4<f32>(position, 1.0)).xyz;
    } else if kind == DARK {
        at = (sky.frames[4] * vec4<f32>(position, 1.0)).xyz;
    }
    out.clip = g.view_proj * vec4<f32>(at, 1.0);
    out.uv = uv;
    out.kind = kind;
    out.model = position;
    return out;
}

// sky.fsh: spherical over 0..sky end, whole once the cylindrical distance reaches it
// (geometry::sky_disc_fog).
fn disc_fog(p: vec3<f32>) -> f32 {
    let end = sky.params.w;
    let cylindrical = max(length(p.xz), abs(p.y));
    return select(clamp(length(p) / end, 0.0, 1.0), 1.0, cylindrical >= end);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let phase = u32(sky.params.y);
    let cell = vec2<f32>(f32(phase % 4u), f32(phase / 4u));
    let sun_color = textureSample(sun, samp, in.uv);
    let moon_color = textureSample(moon, samp, (in.uv + cell) * vec2<f32>(0.25, 0.5));
    // Sampled by level: `fract` breaks the derivatives at each tile's edge.
    let end_color = textureSampleLevel(end_sky, samp, fract(in.uv), 0.0);
    switch in.kind {
        case END: {
            // Java multiplies in sRGB; the sample is linear.
            return vec4<f32>(end_color.rgb * pow(END_SHADE, 2.2), 1.0);
        }
        case SUN: {
            return vec4<f32>(sun_color.rgb, sun_color.a * sky.params.z);
        }
        case MOON: {
            return vec4<f32>(moon_color.rgb, moon_color.a * sky.params.z);
        }
        case STAR: {
            return vec4<f32>(sky.params.x);
        }
        case GLOW: {
            return vec4<f32>(sky.glow.rgb, sky.glow.a * in.uv.x);
        }
        case DOME: {
            return vec4<f32>(mix(sky.zenith.rgb, g.fog.rgb, disc_fog(in.model)), 1.0);
        }
        default: {
            return vec4<f32>(g.fog.rgb * disc_fog(in.model), sky.zenith.w);
        }
    }
}
