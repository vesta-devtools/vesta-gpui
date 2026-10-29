struct SurfaceLighting {
    key: vec4<f32>,
    fill: vec4<f32>,
    sky: vec4<f32>,
    bias: vec4<f32>,
};

// Source normals and diagnostic overlays remain unchanged by this lighting step.
fn lighting_normal(source: vec3<f32>, to_eye: vec3<f32>, face_viewer: bool) -> vec3<f32> {
    let n = normalize(source);
    return select(n, -n, face_viewer && dot(n, to_eye) < 0.0);
}

// The host supplies a normalized, already oriented lighting normal. Source
// normals, sidedness and diagnostic overlays remain the caller's authority.
fn shade_surface(color: vec3<f32>, normal: vec3<f32>, rig: SurfaceLighting) -> vec3<f32> {
    let key = max(dot(normal, normalize(rig.key.xyz)), 0.0);
    let fill = max(dot(normal, normalize(rig.fill.xyz)), 0.0);
    let sky = dot(normal, normalize(rig.sky.xyz)) * 0.5 + 0.5;
    let lighting = rig.bias.w + sky * rig.sky.w + key * rig.key.w + fill * rig.fill.w;
    return color * lighting + rig.bias.xyz;
}
