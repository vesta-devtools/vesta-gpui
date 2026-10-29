struct CameraUniform {
    view_projection: mat4x4<f32>,
    eye: vec4<f32>,
    forward: vec4<f32>,
};

struct ObjectUniform {
    translation: vec4<f32>,
    color: vec4<f32>,
    material: vec4<f32>,
    lighting: SurfaceLighting,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var<uniform> object: ObjectUniform;

struct MeshIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
};

struct MeshOut {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) height: f32,
    @location(3) world: vec3<f32>,
};

@vertex
fn mesh_vertex(input: MeshIn) -> MeshOut {
    var output: MeshOut;
    let world = input.position + object.translation.xyz;
    output.position = camera.view_projection * vec4<f32>(world, 1.0);
    output.normal = input.normal;
    output.color = vec4<f32>(object.color.rgb * input.color, object.color.a);
    output.height = world.y;
    output.world = world;
    return output;
}

@fragment
fn mesh_fragment(input: MeshOut) -> @location(0) vec4<f32> {
    let to_eye = select(-camera.forward.xyz, camera.eye.xyz - input.world, camera.eye.w > 0.5);
    // Two-sided lighting orients only the lighting normal toward the visible
    // side. Source normals and their diagnostic overlays remain unchanged.
    let n = lighting_normal(input.normal, to_eye, object.material.w > 0.5);
    if object.material.z > 0.5 {
        let t = clamp((input.height-object.material.x)/max(object.material.y-object.material.x,1e-6),0.0,1.0);
        let low = vec3<f32>(0.15,0.38,0.45);
        let mid = vec3<f32>(0.42,0.55,0.30);
        let high = vec3<f32>(0.78,0.74,0.58);
        let snowy = vec3<f32>(0.92,0.93,0.95);
        let color = mix(mix(mix(low,mid,smoothstep(0.0,0.45,t)),high,smoothstep(0.45,0.80,t)),snowy,smoothstep(0.80,1.0,t));
        return vec4<f32>(shade_surface(color, n, object.lighting), input.color.a);
    }
    return vec4<f32>(shade_surface(input.color.rgb, n, object.lighting), input.color.a);
}

struct LineIn {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct LineOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn line_vertex(input: LineIn) -> LineOut {
    var output: LineOut;
    output.position = camera.view_projection * vec4<f32>(input.position, 1.0);
    output.color = input.color;
    return output;
}

@fragment
fn line_fragment(input: LineOut) -> @location(0) vec4<f32> {
    return input.color;
}

struct MarkerVertex {
    @location(0) clip_position: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) softness: f32,
};
struct MarkerOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) softness: f32,
};
@vertex
fn marker_vertex(input: MarkerVertex) -> MarkerOutput {
    var result: MarkerOutput;
    result.clip_position = input.clip_position;
    result.uv = input.uv;
    result.color = input.color;
    result.softness = input.softness;
    return result;
}
@fragment
fn marker_fragment(input: MarkerOutput) -> @location(0) vec4<f32> {
    let d = length(input.uv);
    if d > 1.0 { discard; }
    let alpha = 1.0 - smoothstep(1.0 - input.softness, 1.0, d);
    return vec4<f32>(input.color.rgb, input.color.a * alpha);
}
