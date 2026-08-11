struct ViewUniform {
    size: vec2<f32>,
    flash: f32,
    _padding: f32,
};

@group(0) @binding(0)
var source_texture: texture_2d<f32>;
@group(0) @binding(1)
var source_sampler: sampler;
@group(1) @binding(0)
var<uniform> view: ViewUniform;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let positions = array<vec2<f32>, 3>(
        vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0),
    );
    let position = positions[vertex_index];
    var output: VertexOutput;
    output.position = vec4(position, 0.0, 1.0);
    output.uv = vec2((position.x + 1.0) * 0.5, (1.0 - position.y) * 0.5);
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(source_texture, source_sampler, input.uv);
    let rgb = mix(color.rgb, vec3(1.0), clamp(view.flash, 0.0, 1.0));
    return vec4(rgb * color.a, color.a);
}
