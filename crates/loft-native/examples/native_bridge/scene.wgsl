@group(0) @binding(0) var<uniform> camera: vec4f;

@vertex fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
    return vec4f(vec2f(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0, 0.0, 1.0);
}
fn field(p: vec3f) -> f32 {
    let sphere = length(p - vec3f(0.0, 0.12, 0.0)) - 0.70;
    let carved = sphere + 0.043 * sin(13.0*p.x)*sin(13.0*p.y)*sin(13.0*p.z);
    return min(carved, p.y + 0.66);
}
struct Hit {
    @location(0) color: vec4f,
    @builtin(frag_depth) depth: f32,
}
@fragment fn fs(@builtin(position) px: vec4f) -> Hit {
    let xy = ((px.xy + camera.zw) * camera.xy - 0.5) * vec2f(2.8, -1.75);
    let ro = vec3f(0.0, 0.55, 3.0);
    let rd = normalize(vec3f(xy, -2.0));
    var t = 0.0;
    var p = ro;
    for (var i = 0u; i < 120u; i++) {
        p = ro + rd * t;
        let d = field(p);
        if d < 0.0005 || t > 12.0 { break; }
        t += d * 0.48;
    }
    if t >= 12.0 { return Hit(vec4f(0.055, 0.075, 0.12, 1.0), 0.999); }
    let e = vec2f(0.001, 0.0);
    let n = normalize(vec3f(field(p+e.xyy)-field(p-e.xyy), field(p+e.yxy)-field(p-e.yxy),
        field(p+e.yyx)-field(p-e.yyx)));
    let light = normalize(vec3f(-0.6, 0.9, 0.7));
    var shadow = 1.0;
    var s = 0.025;
    for (var i = 0u; i < 56u; i++) {
        let d = field(p + n*0.003 + s*light);
        shadow = min(shadow, clamp(9.0*d/s, 0.0, 1.0));
        if s > 6.0 || d < 0.0001 { break; }
        s += max(d*0.5, 0.003);
    }
    var ao = 0.0;
    for (var i = 1u; i <= 12u; i++) {
        let d = f32(i) * 0.015;
        ao += max(0.0, d - field(p + d*n));
    }
    let copper = mix(vec3f(0.38, 0.13, 0.065), vec3f(0.73, 0.39, 0.16), 0.5+0.5*sin(17.0*p.y));
    let stone = vec3f(0.20, 0.27, 0.30) * (0.87 + 0.13 * cos(5.0*p.x) * cos(5.0*p.z));
    let base = select(copper, stone, p.y < -0.655);
    let diffuse = max(dot(n,light),0.0) * shadow;
    let highlight = pow(max(dot(n, normalize(light-rd)),0.0),38.0) * shadow;
    let rgb = base*(0.26*exp(-5.0*ao)+0.70*diffuse)+0.27*highlight;
    // Monotonic standard [0,1] depth, with the far plane reserved for the background.
    return Hit(vec4f(rgb,1.0), t/(1.0+t));
}
