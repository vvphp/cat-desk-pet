struct ViewUniform {
    size: vec2<f32>,
    flash: f32,
    _padding: f32,
};

@group(0) @binding(0)
var indexed_atlas: texture_2d<u32>;
@group(0) @binding(1)
var palette: texture_2d<f32>;
@group(1) @binding(0)
var<uniform> view: ViewUniform;

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @location(0) local_rect: vec4<f32>,
    @location(1) atlas_rect: vec4<f32>,
    @location(2) matrix: vec4<f32>,
    @location(3) translation: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) atlas_px: vec2<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
        vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
    );
    let corner = corners[input.vertex_index];
    let local = input.local_rect.xy + corner * input.local_rect.zw;
    let physical = vec2(
        input.matrix.x * local.x + input.matrix.y * local.y,
        input.matrix.z * local.x + input.matrix.w * local.y,
    ) + input.translation;
    let ndc = vec2(
        physical.x / view.size.x * 2.0 - 1.0,
        1.0 - physical.y / view.size.y * 2.0,
    );
    var output: VertexOutput;
    output.position = vec4(ndc, 0.0, 1.0);
    output.atlas_px = input.atlas_rect.xy + corner * input.atlas_rect.zw;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let dimensions = vec2<i32>(textureDimensions(indexed_atlas));
    let coordinate = clamp(vec2<i32>(floor(input.atlas_px)), vec2(0), dimensions - vec2(1));
    let indexed = textureLoad(indexed_atlas, coordinate, 0);
    let role = i32(indexed.r);
    if role == 0 || indexed.g == 0 {
        discard;
    }
    let palette_color = textureLoad(palette, vec2(role, 0), 0);
    let alpha = palette_color.a * f32(indexed.g) / 255.0;
    let rgb = mix(palette_color.rgb, vec3(1.0), clamp(view.flash, 0.0, 1.0));
    return vec4(rgb * alpha, alpha);
}
