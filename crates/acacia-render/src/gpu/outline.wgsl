// The targeted block's outline: camera-relative lines. Follows globals.wgsl.

@vertex
fn vs_main(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    return g.view_proj * vec4<f32>(position, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    // Java's LevelRenderer outline: black at 40%.
    return vec4<f32>(0.0, 0.0, 0.0, 0.4);
}
