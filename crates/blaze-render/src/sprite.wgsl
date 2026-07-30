// Blaze Engine — sprite shader (flat colored/UV quad)

struct Ubo {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    color: vec4<f32>,
    _pad: vec4<f32>,
};

@group(0) @binding(0) var<uniform> ubo: Ubo;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) color:    vec4<f32>,
    @location(2) uv:       vec2<f32>,
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let world = ubo.model * vec4<f32>(in.position, 1.0);
    out.position = ubo.view_proj * world;
    out.color = in.color * ubo.color;
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Flat color (no texture binding yet — sprites are colored quads).
    return in.color;
}
