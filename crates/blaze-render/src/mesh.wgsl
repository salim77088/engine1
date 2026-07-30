// Blaze Engine — mesh shader (PBR-ish lighting, directional + point lights)

struct CameraUbo {
    view_proj: mat4x4<f32>,
    view_pos: vec4<f32>,
    _pad: vec4<f32>,
};

struct ModelUbo {
    model: mat4x4<f32>,
    base_color: vec4<f32>,
    roughness: f32,
    metallic: f32,
    emissive: vec4<f32>,
};

struct LightsUbo {
    dir_dir: vec4<f32>,
    dir_color: vec4<f32>,
    dir_intensity: f32,
    _pad0: vec3<f32>,
    point_pos: array<vec4<f32>, 8>,
    point_color: array<vec4<f32>, 8>,
    point_count: f32,
    _pad1: vec3<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUbo;
@group(0) @binding(1) var<uniform> model: ModelUbo;
@group(0) @binding(2) var<uniform> lights: LightsUbo;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal:   vec3<f32>,
    @location(2) color:    vec4<f32>,
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) base_color: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let world = model.model * vec4<f32>(in.position, 1.0);
    out.position = camera.view_proj * world;
    out.world_pos = world.xyz;
    // Note: we ignore non-uniform scaling for normals (acceptable for uniform scale).
    out.normal = normalize((model.model * vec4<f32>(in.normal, 0.0)).xyz);
    out.base_color = in.color * model.base_color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let N = normalize(in.normal);
    let V = normalize(camera.view_pos.xyz - in.world_pos);

    // Ambient
    var ambient = vec3<f32>(0.08, 0.08, 0.10) * in.base_color.rgb;

    // Directional light
    var lit = vec3<f32>(0.0);
    let L_dir = normalize(-lights.dir_dir.xyz);
    let ndotl = max(dot(N, L_dir), 0.0);
    let diff_dir = lights.dir_color.rgb * lights.dir_intensity * ndotl;
    // Half-vector specular (Blinn-Phong, simplified).
    let H_dir = normalize(L_dir + V);
    let spec_dir = pow(max(dot(N, H_dir), 0.0), 32.0) * lights.dir_intensity;
    lit += (diff_dir + spec_dir) * in.base_color.rgb;

    // Point lights
    let count = u32(lights.point_count);
    for (var i: u32 = 0u; i < count; i = i + 1u) {
        let pl_pos = lights.point_pos[i].xyz;
        let pl_color = lights.point_color[i].rgb;
        let pl_intensity = lights.point_color[i].w;
        let to_light = pl_pos - in.world_pos;
        let dist = length(to_light);
        let L = to_light / max(dist, 0.0001);
        let attenuation = 1.0 / max(dist * dist, 0.01);
        let ndotl_p = max(dot(N, L), 0.0);
        let diff_p = pl_color * pl_intensity * ndotl_p * attenuation;
        let H_p = normalize(L + V);
        let spec_p = pow(max(dot(N, H_p), 0.0), 32.0) * pl_intensity * attenuation;
        lit += (diff_p + spec_p) * in.base_color.rgb;
    }

    let final_color = ambient + lit + model.emissive.rgb;
    return vec4<f32>(final_color, in.base_color.a);
}
