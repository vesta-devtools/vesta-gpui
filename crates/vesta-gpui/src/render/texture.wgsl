struct TextureIn {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct TextureOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;

@vertex
fn vertex(input: TextureIn) -> TextureOut {
    var output: TextureOut;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.uv = input.uv;
    return output;
}

@fragment
fn fragment(input: TextureOut) -> @location(0) vec4<f32> {
    return textureSample(image, image_sampler, input.uv);
}
