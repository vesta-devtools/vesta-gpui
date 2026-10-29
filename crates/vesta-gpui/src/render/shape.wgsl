struct ShapeIn {
    @location(0) position: vec2<f32>,
    @location(1) local: vec2<f32>,
    @location(2) size: vec2<f32>,
    @location(3) radius_border: vec2<f32>,
    @location(4) color: vec4<f32>,
};

struct ShapeOut {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) radius_border: vec2<f32>,
    @location(3) color: vec4<f32>,
};

@vertex
fn vertex(input: ShapeIn) -> ShapeOut {
    var output: ShapeOut;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.local = input.local;
    output.size = input.size;
    output.radius_border = input.radius_border;
    output.color = input.color;
    return output;
}

fn rounded_distance(point: vec2<f32>, size: vec2<f32>, radius: f32) -> f32 {
    let half_size = size * 0.5;
    let q = abs(point - half_size) - (half_size - vec2<f32>(radius));
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

@fragment
fn fragment(input: ShapeOut) -> @location(0) vec4<f32> {
    let radius = input.radius_border.x;
    let border = input.radius_border.y;
    var coverage = 1.0;
    if (radius >= 0.0) {
        let outer_distance = rounded_distance(input.local, input.size, radius);
        let outer_width = max(fwidth(outer_distance), 0.0001);
        coverage = clamp(0.5 - outer_distance / outer_width, 0.0, 1.0);
        if (border > 0.0) {
            let inner_size = input.size - vec2<f32>(border * 2.0);
            let inner_point = input.local - vec2<f32>(border);
            if (inner_size.x > 0.0 && inner_size.y > 0.0) {
                let inner_distance = rounded_distance(inner_point, inner_size, max(radius - border, 0.0));
                let inner_width = max(fwidth(inner_distance), 0.0001);
                coverage *= clamp(0.5 + inner_distance / inner_width, 0.0, 1.0);
            }
        }
    }
    return vec4<f32>(input.color.rgb, input.color.a * coverage);
}
