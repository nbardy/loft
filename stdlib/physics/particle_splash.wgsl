struct StateRow { position: vec4f, velocity: vec4f }
@group(1) @binding(0) var<storage, read> State: array<StateRow>;

struct Vertex {
    @builtin(position) clip: vec4f,
    @location(0) uv: vec2f,
    @location(1) color: vec3f,
    @location(2) @interpolate(flat) layer: u32,
}
const corners = array(vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(-1.0, 1.0),
                      vec2f(-1.0, 1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0));

fn project(p: vec3f) -> vec2f {
    let d = p - vec3f(16.0, 0.0, 16.0);
    return vec2f(0.043 * d.x - 0.023 * d.z, 0.047 * d.y + 0.016 * d.x + 0.024 * d.z - 0.58);
}

@vertex fn vs(@builtin(vertex_index) vertex: u32, @builtin(instance_index) instance: u32) -> Vertex {
    let uv = corners[vertex];
    if instance == 0u {
        return Vertex(vec4f(uv, 0.9, 1.0), uv, vec3f(0.0), 0u);
    }
    if instance == 1u {
        let p = vec3f(mix(vec2f(1.25), vec2f(29.75), uv * 0.5 + 0.5), 0.0).xzy;
        return Vertex(vec4f(project(p), 0.8, 1.0), p.xz, vec3f(0.0), 1u);
    }
    let row = State[instance - 2u];
    let p = row.position.xyz;
    let speed = length(row.velocity.xyz);
    let shade = clamp(0.37 + 0.027 * p.y + 0.025 * (p.z - 15.0), 0.0, 1.0);
    let deep = vec3f(0.013, 0.12, 0.24);
    let aqua = vec3f(0.05, 0.64, 0.82);
    let foam = vec3f(0.71, 0.95, 1.0);
    let ink = mix(mix(deep, aqua, shade), foam, smoothstep(7.0, 17.0, speed) * 0.72);
    let radius = mix(0.0024, 0.0041, shade);
    let clip = project(p) + uv * vec2f(radius * 0.625, radius);
    return Vertex(vec4f(clip, 0.5, 1.0), uv, ink, 2u);
}

@fragment fn fs(v: Vertex) -> @location(0) vec4f {
    if v.layer == 0u {
        let halo = clamp(1.0 - length((v.uv - vec2f(-0.05, 0.06)) * vec2f(0.68, 0.9)), 0.0, 1.0);
        return vec4f(vec3f(0.01, 0.016, 0.026) + halo * vec3f(0.016, 0.032, 0.052), 1.0);
    }
    if v.layer == 1u {
        let edge = min(v.uv - 1.25, 29.75 - v.uv);
        let seam = abs(fract(v.uv / 4.0 + 0.5) - 0.5);
        let line = 1.0 - smoothstep(0.012, 0.028, min(seam.x, seam.y));
        let rim = 1.0 - smoothstep(0.09, 0.23, min(edge.x, edge.y));
        return vec4f(vec3f(0.023, 0.046, 0.064) + line * 0.025 + rim * vec3f(0.04, 0.12, 0.16), 1.0);
    }
    let r2 = dot(v.uv, v.uv);
    if r2 > 1.0 { discard; }
    let gloss = pow(max(0.0, dot(vec3f(v.uv, sqrt(1.0 - r2)), vec3f(-0.35, 0.55, 0.76))), 6.0);
    return vec4f(v.color * (0.71 + 0.29 * (1.0 - r2)) + vec3f(0.15, 0.22, 0.24) * gloss, 1.0);
}
