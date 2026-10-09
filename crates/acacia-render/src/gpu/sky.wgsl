// Sun, moon and stars, added onto the cleared sky before the terrain. Follows globals.wgsl.

struct Celestial {
    // The sky's turn around the east-west axis.
    turn: mat4x4<f32>,
    // x: star brightness, y: moon phase (0..8), z: sun and moon visibility (rain hides them),
    // w: the side the glow is on (+1 east, -1 west)
    params: vec4<f32>,
    // The sunrise or sunset glow: linear rgb and its strength (0 for none)
    glow: vec4<f32>,
    // rgb: the sky's colour overhead (linear); the frame is cleared to the fog's
    zenith: vec4<f32>,
};

@group(0) @binding(1) var<uniform> sky: Celestial;
@group(0) @binding(2) var sun: texture_2d<f32>;
@group(0) @binding(3) var moon: texture_2d<f32>;
@group(0) @binding(4) var samp: sampler;
@group(0) @binding(5) var end_sky: texture_2d<f32>;

const SUN: u32 = 0u;
const MOON: u32 = 1u;
// A fan around the horizon (Java's sunrise disc): position is (cos, sin, 1) on the rim and
// (_, _, 0) at its centre, which sits on the horizon under the sun.
const GLOW: u32 = 3u;
// Java's sky disc: flat, 16 blocks overhead, fading into the fog with distance.
const DOME: u32 = 4u;
// The End's sky (Java's `renderEndSky`): a box round the camera, its texture tiled 16 times a face.
const END: u32 = 5u;
const END_TILES: f32 = 16.0;
const END_SHADE: f32 = 0.1568627;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) kind: u32,
    @location(2) rel: vec3<f32>,
};

@vertex
fn vs_main(@location(0) position: vec3<f32>, @location(1) kind: u32, @location(2) uv: vec2<f32>) -> VsOut {
    var out: VsOut;
    out.clip = g.view_proj * vec4<f32>((sky.turn * vec4<f32>(position, 0.0)).xyz, 1.0);
    out.uv = uv;
    out.kind = kind;
    out.rel = position;
    if kind == DOME || kind == END {
        out.clip = g.view_proj * vec4<f32>(position, 1.0);
    }
    if kind == GLOW {
        // The rim lifts on the sun's side by the glow's strength and fades to nothing.
        let rim = vec3<f32>(sky.params.w * position.x * 120.0, position.x * 40.0 * sky.glow.a, position.y * 120.0);
        let at = mix(vec3<f32>(sky.params.w * 100.0, 0.0, 0.0), rim, position.z);
        out.clip = g.view_proj * vec4<f32>(at, 1.0);
        out.uv = vec2<f32>(1.0 - position.z, 0.0);
    }
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let phase = u32(sky.params.y);
    let cell = vec2<f32>(f32(phase % 4u), f32(phase / 4u));
    let sun_color = textureSample(sun, samp, in.uv);
    let moon_color = textureSample(moon, samp, (in.uv + cell) * vec2<f32>(0.25, 0.5));
    // Sampled by level: `fract` breaks the derivatives at each tile's edge.
    let end_color = textureSampleLevel(end_sky, samp, fract(in.uv * END_TILES), 0.0);
    if in.kind == END {
        // Java multiplies in sRGB; the sample is linear.
        return vec4<f32>(end_color.rgb * pow(END_SHADE, 2.2), 1.0);
    }
    if in.kind == SUN {
        return sun_color * sky.params.z;
    }
    if in.kind == MOON {
        return moon_color * sky.params.z;
    }
    if in.kind == GLOW {
        return vec4<f32>(sky.glow.rgb, sky.glow.a * in.uv.x);
    }
    if in.kind == DOME {
        return vec4<f32>(mix(sky.zenith.rgb, g.fog.rgb, clamp(length(in.rel) / g.fog.w, 0.0, 1.0)), 1.0);
    }
    return vec4<f32>(vec3<f32>(sky.params.x), 1.0);
}
