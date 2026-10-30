struct CameraUniform { view_projection: mat4x4<f32> };
@group(0) @binding(0) var<uniform> camera: CameraUniform;
struct Source { position: vec3<f32>, pad0: f32, number: u32, pad1: u32, pad2: u32, pad3: u32 };
struct Policy {
    viewport: vec4<f32>, // physical x/y/width/height
    geometry: vec4<f32>, // physical cell size, glyph scale, advance fraction
    color: vec4<f32>,
    placement: vec4<f32>, // physical offset, digit capacity, atlas cells
    depth: vec4<f32>, // tolerance, sample count, required occluded samples, unused
    clip: vec4<f32>, // this view's physical visible depth region
};
@group(1) @binding(0) var<storage, read> sources: array<Source>;
@group(1) @binding(1) var<uniform> policy: Policy;
@group(1) @binding(2) var atlas: texture_2d<f32>;
@group(1) @binding(3) var atlas_sampler: sampler;
@group(2) @binding(0) var label_depth: texture_depth_multisampled_2d;

struct Output {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) digit: f32,
    @location(2) @interpolate(flat) anchor: vec3<f32>,
};
fn corner(index: u32) -> vec2<f32> {
    switch index {
        case 0u, 3u: { return vec2<f32>(-1.0, -1.0); }
        case 1u: { return vec2<f32>(1.0, -1.0); }
        case 2u, 4u: { return vec2<f32>(1.0, 1.0); }
        default: { return vec2<f32>(-1.0, 1.0); }
    }
}
@vertex
fn label_vertex(@builtin(vertex_index) index: u32) -> Output {
    let per_source = u32(policy.placement.z) * 6u;
    let source = sources[index / per_source];
    let quad = (index % per_source) / 6u;
    let clip = camera.view_projection * vec4<f32>(source.position, 1.0);
    var output: Output;
    output.clip_position = vec4<f32>(0.0, 0.0, 2.0, 1.0);
    output.uv = vec2<f32>(0.0); output.digit = 0.0; output.anchor = vec3<f32>(0.0);
    if clip.w <= 0.0 { return output; }
    var count = 1u; var value = source.number;
    while value >= 10u { value = value / 10u; count = count + 1u; }
    if quad >= count { return output; }
    value = source.number;
    for(var k = 0u; k < quad; k = k + 1u) { value = value / 10u; }
    let local = corner(index % 6u);
    let width = policy.geometry.x * policy.geometry.z;
    let x = policy.placement.x + width * 0.5
        + f32(count - 1u - quad) * policy.geometry.x * policy.geometry.w;
    let offset = vec2<f32>(x + local.x * width * 0.5, policy.placement.y + local.y * policy.geometry.y * 0.5);
    output.clip_position = vec4<f32>(clip.xy + offset * 2.0 / policy.viewport.zw * clip.w, 0.0, clip.w);
    output.uv = local; output.digit = f32(value % 10u); output.anchor = clip.xyz / clip.w;
    return output;
}
fn sample_label_depth(pixel: vec2<i32>, sample: i32) -> f32 {
    return textureLoad(label_depth, pixel, sample);
}
@fragment
fn label_fragment(input: Output) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(policy.viewport.xy + vec2<f32>(input.anchor.x * 0.5 + 0.5, 0.5 - input.anchor.y * 0.5) * policy.viewport.zw);
    let clip_min = vec2<i32>(policy.clip.xy);
    let clip_max = clip_min + vec2<i32>(policy.clip.zw);
    // Never sample a different view's depth for an anchor outside this clip.
    if pixel.x < clip_min.x || pixel.y < clip_min.y || pixel.x >= clip_max.x || pixel.y >= clip_max.y { discard; }
    let size = vec2<i32>(textureDimensions(label_depth));
    if pixel.x < 0 || pixel.y < 0 || pixel.x >= size.x || pixel.y >= size.y { discard; }
    var occluded = 0u;
    for(var sample = 0; sample < i32(policy.depth.y); sample = sample + 1) {
        if input.anchor.z - policy.depth.x > sample_label_depth(pixel, sample) { occluded = occluded + 1u; }
    }
    if occluded >= u32(policy.depth.z) { discard; }
    let uv = vec2<f32>((input.digit + (input.uv.x * 0.5 + 0.5) * policy.geometry.z + (1.0 - policy.geometry.z) * 0.5) / policy.placement.w,
        0.5 - input.uv.y * 0.5);
    let glyph = textureSample(atlas, atlas_sampler, uv);
    if glyph.a < 0.05 { discard; }
    return vec4<f32>(policy.color.rgb, policy.color.a * glyph.a);
}
