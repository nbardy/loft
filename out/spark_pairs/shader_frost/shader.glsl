// Frost crystal / voronoi shader — base (animated) variant.
// GLSL ES fragment, Shadertoy-style uniforms. Self-contained: no textures.
//   uniform vec2  u_res;   // viewport in pixels
//   uniform float u_time;  // seconds
//
// Look: dendritic frost veins drawn from the voronoi edge distance (F2-F1),
// papered over a pale ice field, with thin-film iridescence keyed to cell id
// plus edge distance, and a fine frost-grain sparkle.
//
// Variants live beside this file:
//   shader_static.glsl — STATIC=1 frozen frame, denser cells, crisper veins.
//   shader_lit.glsl    — adds a key/fill light pair over voronoi-height normals.
precision mediump float;

uniform vec2 u_res;
uniform float u_time;

#ifndef CELLS
#define CELLS 5.0
#endif
#ifndef VEIN_W
#define VEIN_W 0.09
#endif
#ifndef IRIDESCENCE
#define IRIDESCENCE 1.0
#endif
#ifndef GRAIN
#define GRAIN 0.6
#endif

vec2 hash22(vec2 p) {
  vec3 q = fract(vec3(p.xyx) * vec3(0.1031, 0.1030, 0.0973));
  q += dot(q, q.yzx + 33.33);
  return fract((q.xx + q.yz) * q.zy);
}

// Returns F1, F2 and the nearest-cell id. 3x3 window, one jittered site/cell.
vec3 voro(vec2 p, out vec2 cellId) {
  vec2 ip = floor(p);
  vec2 fp = fract(p);
  float f1 = 8.0;
  float f2 = 8.0;
  vec2 id = vec2(0.0);
  for (int j = -1; j <= 1; j++) {
    for (int i = -1; i <= 1; i++) {
      vec2 g = vec2(float(i), float(j));
      vec2 o = hash22(ip + g);
      vec2 r = g + o - fp;
      float d = dot(r, r);
      if (d < f1) {
        f2 = f1;
        f1 = d;
        id = ip + g;
      } else if (d < f2) {
        f2 = d;
      }
    }
  }
  cellId = id;
  return vec3(sqrt(f1), sqrt(f2), 0.0);
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

// Thin-film approx: hue cycles with film thickness + view of edge distance.
vec3 thinfilm(float t) {
  return 0.5 + 0.5 * cos(6.2831853 * (t * vec3(1.0, 1.15, 1.32) + vec3(0.0, 0.33, 0.67)));
}

void main() {
  vec2 uv = gl_FragCoord.xy / u_res;
  vec2 p = (uv - 0.5) * vec2(u_res.x / u_res.y, 1.0);
  float t = u_time;

  // Slow drift so veins creep like growing frost; static variant freezes t = 0.
  vec2 wp = p * CELLS + 0.05 * vec2(vnoise(p * 3.0 + t * 0.05), vnoise(p * 3.0 + 7.7 - t * 0.04));

  vec2 cid;
  vec3 v = voro(wp, cid);
  float edge = v.y - v.x;                       // F2-F1: 0 on cell borders
  float vein = 1.0 - smoothstep(0.0, VEIN_W, edge);

  float cellHash = fract(sin(dot(cid, vec2(127.1, 311.7))) * 43758.5453);
  vec3 irid = thinfilm(v.x * 1.4 + cellHash) * IRIDESCENCE;

  // Pale ice field, cooler in cell centres, brighter toward veins.
  vec3 field = mix(vec3(0.62, 0.72, 0.82), vec3(0.82, 0.89, 0.95), vein);
  field += 0.10 * (v.x - 0.5);

  // Frost grain sparkle, animated shimmer.
  float g = vnoise(gl_FragCoord.xy * 0.55 + t * 2.0) * vnoise(gl_FragCoord.xy * 0.23 - t * 1.3);
  float sparkle = smoothstep(0.055, 0.0, abs(g - 0.25)) * GRAIN;

  vec3 col = field + vein * (0.25 + 0.75 * irid);
  col += sparkle * (0.35 + 0.65 * vein);

  // Gentle vignette to hold the macro-photo framing.
  vec2 q = uv - 0.5;
  col *= 1.0 - 0.45 * dot(q, q);

  gl_FragColor = vec4(col, 1.0);
}
