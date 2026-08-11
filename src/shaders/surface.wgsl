@group(0) @binding(0)
var premultiplied_frame: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let positions = array<vec2<f32>, 3>(
        vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0),
    );
    var output: VertexOutput;
    output.position = vec4(positions[vertex_index], 0.0, 1.0);
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let dimensions = vec2<i32>(textureDimensions(premultiplied_frame));
    let coordinate = clamp(vec2<i32>(input.position.xy), vec2(0), dimensions - vec2(1));
    let color = textureLoad(premultiplied_frame, coordinate, 0);
    if color.a <= 0.00001 {
        return vec4(0.0);
    }
    return vec4(clamp(color.rgb / color.a, vec3(0.0), vec3(1.0)), color.a);
}
