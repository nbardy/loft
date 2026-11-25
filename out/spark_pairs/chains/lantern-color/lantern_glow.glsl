// lantern-color contrast sample: the same warm->cool axis as chain.loft,
// expressed as a 2D glow field. CPU-mirrorable (see frost render.py idiom).
// uWarm 1 = amber cozy, 0 = moonlight cold. Step t maps to uWarm = 1-t/10.
precision mediump float;
uniform vec2 uRes;
uniform float uWarm;   // 1..0 across steps 0..10
uniform float uTime;

vec3 warmGlow = vec3(1.0, 0.62, 0.20);
vec3 coolGlow = vec3(0.55, 0.70, 1.00);

void main() {
  vec2 uv = (gl_FragCoord.xy - 0.5 * uRes) / uRes.y;
  float r = length(uv);
  float halo = exp(-r * r * 9.0);
  float flicker = 0.96 + 0.04 * sin(uTime * 2.0 + r * 14.0);
  vec3 glow = mix(coolGlow, warmGlow, uWarm);
  vec3 paper = mix(vec3(0.88, 0.91, 0.98), vec3(0.98, 0.95, 0.90), uWarm);
  float body = smoothstep(0.30, 0.26, abs(r - 0.19) - 0.10);
  vec3 col = mix(vec3(0.91, 0.90, 0.88), paper * (0.35 + halo) + glow * halo * flicker, body);
  col += glow * halo * 0.55 * flicker * (1.0 - body);
  gl_FragColor = vec4(col, 1.0);
}
