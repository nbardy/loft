// Frost crystal — LIT variant (key/fill lights over voronoi-height normals).
// Same frost veins + iridescence as shader.glsl, plus a simple lighting model:
// height h = F1 is treated as a relief field, normals come from finite
// differences of h, and a cool key + warm fill shade the albedo. Veins sit
// proud of the surface (raised h) so they catch the key highlight.
precision mediump float;

uniform vec2 u_res;
uniform float u_time;

#ifndef CELLS
#define CELLS 5.0
#endif
#ifndef VEIN_W
#define VEIN_W 0.09
#endif

vec2 hash22(vec2 p) {
  vec3 q = fract(vec3(p.xyx) * vec3(0.1031, 0.1030, 0.0973));
  q += dot(q, q.yzx + 33.33);
  return fract((q.xx + q.yz) * q.zy);
}

vec2 voro12(vec2 p) {
  vec2 ip = floor(p);
  vec2 fp = fract(p);
  float f1 = 8.0;
  float f2 = 8.0;
  for (int j = -1; j <= 1; j++) {
    for (int i = -1; i <= 1; i++) {
      vec2 g = vec2(float(i), float(j));
      vec2 o = hash22(ip + g);
      vec2 r = g + o - fp;
      float d = dot(r, r);
      if (d < f1) {
        f2 = f1;
        f1 = d;
      } else if (d < f2) {
        f2 = d;
      }
    }
  }
  return vec2(sqrt(f1), sqrt(f2));
}

float vnoise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  float a = fract(sin(dot(i, vec2(127.1, 311.7))) * 43758.5453);
  float b = fract(sin(dot(i + vec2(1.0, 0.0), vec2(127.1, 311.7))) * 43758.5453);
  float c = fract(sin(dot(i + vec2(0.0, 1.0), vec2(127.1, 311.7))) * 43758.5453);
  float d = fract(sin(dot(i + vec2(1.0, 1.0), vec2(127.1, 311.7))) * 43758.5453);
  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

vec3 thinfilm(float t) {
  return 0.5 + 0.5 * cos(6.2831853 * (t * vec3(1.0, 1.15, 1.32) + vec3(0.0, 0.33, 0.67)));
}

// Relief height: cell-centre domes with veins raised proud.
float relief(vec2 wp) {
  vec2 v = voro12(wp);
  float edge = v.y - v.x;
  float vein = 1.0 - smoothstep(0.0, VEIN_W, edge);
  return v.x * 0.55 + vein * 0.30;
}

void main() {
  vec2 uv = gl_FragCoord.xy / u_res;
  vec2 aspect = vec2(u_res.x / u_res.y, 1.0);
  vec2 p = (uv - 0.5) * aspect;
  float t = u_time;

  vec2 wp = p * CELLS + 0.05 * vec2(vnoise(p * 3.0 + t * 0.05), vnoise(p * 3.0 + 7.7 - t * 0.04));

  vec2 v = voro12(wp);
  float edge = v.y - v.x;
  float vein = 1.0 - smoothstep(0.0, VEIN_W, edge);
  vec3 irid = thinfilm(v.x * 1.4 + fract(v.x * 12.9) * 0.37);

  vec3 albedo = mix(vec3(0.55, 0.66, 0.78), vec3(0.80, 0.88, 0.95), vein);
  albedo += vein * 0.55 * irid;

  // Normals from relief gradient (finite differences in warped space).
  float e = 0.012;
  float h0 = relief(wp);
  float hx = relief(wp + vec2(e, 0.0));
  float hy = relief(wp + vec2(0.0, e));
  float bump = 2.2;
  vec3 n = normalize(vec3(-(hx - h0) * bump / e * 0.12, -(hy - h0) * bump / e * 0.12, 1.0));

  vec3 key = normalize(vec3(-0.55, 0.62, 0.55));   // cool key, upper left
  vec3 fill = normalize(vec3(0.6, -0.25, 0.45));   // warm bounce, lower right
  vec3 vv = vec3(0.0, 0.0, 1.0);

  float dif = max(dot(n, key), 0.0);
  float fil = max(dot(n, fill), 0.0);
  vec3 h = normalize(key + vv);
  float spec = pow(max(dot(n, h), 0.0), 60.0) * (0.35 + 0.65 * vein);

  vec3 col = albedo * (0.38 + 0.72 * dif * vec3(0.85, 0.93, 1.0) + 0.25 * fil * vec3(1.0, 0.85, 0.7));
  col += spec * vec3(0.9, 0.96, 1.0);

  vec2 q = uv - 0.5;
  col *= 1.0 - 0.45 * dot(q, q);

  gl_FragColor = vec4(col, 1.0);
}
