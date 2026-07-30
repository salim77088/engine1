@vertex
fn vs_main(@location(0) position: vec3<f32>,
           @location(1) color:    vec3<f32>) -> BuiltInOutput {
    var out: BuiltInOutput;
    out.position = vec4<f32>(position, 1.0);
    out.color = color;
    return out;
}

struct BuiltInOutput {
    @builtin(position) position: vec4<f32>,
    @location(0)       color:    vec3<f32>,
};

@fragment
fn fs_main(in: BuiltInOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
