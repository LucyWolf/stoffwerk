struct Globals {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
};

@vertex
fn vs_main(v: VsIn) -> VsOut {
    var o: VsOut;
    o.clip = g.view_proj * vec4<f32>(v.pos, 1.0);
    o.world = v.pos;
    o.normal = v.normal;
    o.color = v.color;
    return o;
}

// Weiches Studiolicht: Hauptlicht schräg von vorn oben, Gegenlicht, Himmel/Boden-Umgebung.
@fragment
fn fs_lit(i: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    var n = normalize(i.normal);
    if (!front) { n = -n; }
    let view_dir = normalize(g.eye.xyz - i.world);
    let key = normalize(vec3<f32>(0.4, 0.8, 0.6));
    let rim = normalize(vec3<f32>(-0.5, 0.3, -0.8));
    let hemi = mix(vec3<f32>(0.32, 0.30, 0.28), vec3<f32>(0.55, 0.58, 0.65), n.y * 0.5 + 0.5);
    let diff = max(dot(n, key), 0.0) * 0.75 + max(dot(n, rim), 0.0) * 0.25;
    let h = normalize(key + view_dir);
    let spec = pow(max(dot(n, h), 0.0), 32.0) * 0.12;
    let c = i.color * (hemi + vec3<f32>(diff)) + vec3<f32>(spec);
    return vec4<f32>(c, 1.0);
}

@fragment
fn fs_flat(i: VsOut) -> @location(0) vec4<f32> {
    // Gitter blendet zum Rand hin aus
    let d = length(i.world.xz);
    let fade = clamp(1.0 - d / 5.0, 0.0, 1.0);
    return vec4<f32>(i.color, fade);
}
