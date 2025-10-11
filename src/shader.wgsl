@group(0) @binding(0)
var<uniform> screen: vec2<f32>;

@vertex
fn vs_main(
    @location(0) vertex: vec2<f32>,
) -> @builtin(position) vec4<f32> {

    return vec4<f32>((vertex.x * 2.0) / screen.x - 1.0, (vertex.y * 2.0) / screen.y - 1.0, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) in: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
