// Lightning bolts: untextured triangles added onto the frame. Follows globals.wgsl.

const COLOR: vec4<f32> = vec4<f32>(0.45, 0.45, 0.5, 0.3);

@vertex
fn vs_main(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    return g.view_proj * vec4<f32>(position, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return COLOR;
}
