// Regenerate with bun crates/loft-passes/src/bsdf/energy.js > crates/loft-passes/src/bsdf/energy.in.
// Heitz's GGX visible-normal sampling changes the unit-Fresnel integral to
// the mean of G2(v,l)/G1(v) for reflections above the hemisphere.
const intervals = 32;
const samples = 16384;
const disk = Array.from({length: samples}, (_, n) => {
  let bits = n;
  let inverse = 0;
  while (bits) {
    inverse = (inverse + (bits & 1)) / 2;
    bits >>>= 1;
  }
  const radius = Math.sqrt((n + 0.5) / samples);
  const angle = 2 * Math.PI * inverse;
  return [radius * Math.cos(angle), radius * Math.sin(angle)];
});

function directional(alpha, mu) {
  if (mu === 0) return 1;
  if (alpha === 1) return 1 - mu * Math.log1p(1 / mu);
  const vx = Math.sqrt(1 - mu * mu);
  const stretched = Math.hypot(alpha * vx, mu);
  const x = alpha * vx / stretched;
  const z = mu / stretched;
  let sum = 0;
  for (const [u, v] of disk) {
    const p = (1 - z) * 0.5;
    const y = p * Math.sqrt(1 - u * u) + (1 - p) * v;
    const w = Math.sqrt(Math.max(0, 1 - u * u - y * y));
    const hx = alpha * (x * w - z * y);
    const hy = alpha * u;
    const hz = z * w + x * y;
    const wi = 2 * (vx * hx + mu * hz) * hz / (hx * hx + hy * hy + hz * hz) - mu;
    if (wi > 0) {
      const lambda = (Math.hypot(alpha * Math.sqrt(Math.max(0, 1 - wi * wi)), wi) / wi - 1) / 2;
      sum += (stretched + mu) / (stretched + mu + 2 * mu * lambda);
    }
  }
  return sum / samples;
}

function average(row) {
  // E is linear in x = sqrt(mu) inside a cell. Integrate 4*x^3*E with
  // three-point Gauss, exact here because the product is quartic.
  let integral = 0;
  for (let i = 0; i < intervals; i++) {
    for (const [xi, weight] of [[-Math.sqrt(0.6), 5/9], [0, 8/9], [Math.sqrt(0.6), 5/9]]) {
      const t = (xi + 1) / 2;
      const x = (i + t) / intervals;
      const e = (1 - t) * row[i] + t * row[i + 1];
      integral += 2 * x ** 3 * e * weight / intervals;
    }
  }
  return integral;
}

console.log("[");
for (let i = 0; i <= intervals; i++) {
  const alpha = Math.max(0.002, (i / intervals) ** 2);
  const row = Array.from({length: intervals + 1}, (_, j) => directional(alpha, (j / intervals) ** 2));
  row.push(average(row));
  console.log("    " + row.map(e => Math.round(e * 65535)).join(", ") + ",");
}
console.log("]");
