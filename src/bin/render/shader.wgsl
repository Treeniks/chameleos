struct Uniform {
    screen_size: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> info: Uniform;

struct VertexInput {
    @location(0) pos: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.pos = vec4<f32>(
        (in.pos.x * 2.0) / info.screen_size.x - 1.0,
        (in.pos.y * 2.0) / info.screen_size.y - 1.0,
        0.0, 1.0,
    );
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
