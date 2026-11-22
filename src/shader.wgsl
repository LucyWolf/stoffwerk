struct Globals {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
};

struct Material {
    // x: Ausstanzen (1 = Pixel unter 50 % Deckkraft weglassen), y: Textur vorhanden
    flags: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;
@group(1) @binding(2) var<uniform> mat: Material;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) uv: vec2<f32>,
};

@vertex
fn vs_main(v: VsIn) -> VsOut {
    var o: VsOut;
    o.clip = g.view_proj * vec4<f32>(v.pos, 1.0);
    o.world = v.pos;
    o.normal = v.normal;
    o.color = v.color;
    o.uv = v.uv;
    return o;
}

// Weiches Studiolicht: Hauptlicht schräg von vorn oben, Gegenlicht, Himmel/Boden-Umgebung.
@fragment
fn fs_lit(i: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    // flags.y: Teil hat eine Textur; sonst gilt nur die Materialfarbe
    let sampled = textureSample(tex, samp, i.uv);
    let t = select(vec4<f32>(1.0, 1.0, 1.0, 1.0), sampled, mat.flags.y > 0.5);
    if (mat.flags.x > 0.5 && t.a < 0.5) {
        discard;
    }
    var n = normalize(i.normal);
    if (!front) { n = -n; }
    let view_dir = normalize(g.eye.xyz - i.world);
    let key = normalize(vec3<f32>(0.4, 0.8, 0.6));
    let rim = normalize(vec3<f32>(-0.5, 0.3, -0.8));
    let hemi = mix(vec3<f32>(0.32, 0.30, 0.28), vec3<f32>(0.55, 0.58, 0.65), n.y * 0.5 + 0.5);
    let diff = max(dot(n, key), 0.0) * 0.75 + max(dot(n, rim), 0.0) * 0.25;
    let h = normalize(key + view_dir);
    let spec = pow(max(dot(n, h), 0.0), 32.0) * 0.08;
    let albedo = i.color * t.rgb;
    let c = albedo * (hemi + vec3<f32>(diff)) + vec3<f32>(spec);
    return vec4<f32>(c, 1.0);
}

@fragment
fn fs_flat(i: VsOut) -> @location(0) vec4<f32> {
    // Gitter blendet zum Rand hin aus
    let d = length(i.world.xz);
    let fade = clamp(1.0 - d / 5.0, 0.0, 1.0);
    return vec4<f32>(i.color, fade);
}
