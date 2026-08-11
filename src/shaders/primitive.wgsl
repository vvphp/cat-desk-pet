struct ViewUniform {
    size: vec2<f32>,
    flash: f32,
    _padding: f32,
};

@group(0) @binding(0)
var<uniform> view: ViewUniform;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4(
        input.position.x / view.size.x * 2.0 - 1.0,
        1.0 - input.position.y / view.size.y * 2.0,
        0.0,
        1.0,
    );
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let alpha = input.color.a;
    let rgb = mix(input.color.rgb, vec3(1.0), clamp(view.flash, 0.0, 1.0));
    return vec4(rgb * alpha, alpha);
}
