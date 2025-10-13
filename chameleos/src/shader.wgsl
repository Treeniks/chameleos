struct Uniform {
    stroke_color: vec4<f32>,
    screen_size: vec2<f32>,
    // unused
    _fill: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> info: Uniform;

@vertex
fn vs_main(
    @location(0) vertex: vec2<f32>,
) -> @builtin(position) vec4<f32> {
    return vec4<f32>(
        (vertex.x * 2.0) / info.screen_size.x - 1.0,
        (vertex.y * 2.0) / info.screen_size.y - 1.0,
        0.0, 1.0,
    );
}

@fragment
fn fs_main(@builtin(position) in: vec4<f32>) -> @location(0) vec4<f32> {
    return info.stroke_color;
}
