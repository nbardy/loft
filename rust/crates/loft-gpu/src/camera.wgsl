// The one camera builder (plan §1.5): a lens (loft_ir::frame::LENS, written by a bundle camera's lens entry or by the
// host) to the frame header's camera rows: clip from world, world from clip, the eye and the forward direction.
// A look camera follows TS src/host/camera.ts (R0's): near and far at distance / 100 and × 100, depth 0..1, and a
// reference up that flips to -z (or +z) when the view looks along it.
struct Head { view_proj: mat4x4<f32>, inv_view_proj: mat4x4<f32>, eye: vec4<f32>, forward: vec4<f32>, viewport: vec4<f32> }
@group(0) @binding(0) var<uniform> frame: Head;
@group(1) @binding(0) var<storage, read> lens: array<vec4<f32>, 5>;
@group(1) @binding(1) var<storage, read_write> head: array<vec4<f32>, 10>;

// General inverse by cofactors (as camera.ts, term for term).
fn invert(m: mat4x4<f32>) -> mat4x4<f32> {
  let a00 = m[0][0]; let a01 = m[0][1]; let a02 = m[0][2]; let a03 = m[0][3];
  let a10 = m[1][0]; let a11 = m[1][1]; let a12 = m[1][2]; let a13 = m[1][3];
  let a20 = m[2][0]; let a21 = m[2][1]; let a22 = m[2][2]; let a23 = m[2][3];
  let a30 = m[3][0]; let a31 = m[3][1]; let a32 = m[3][2]; let a33 = m[3][3];
  let b00 = a00 * a11 - a01 * a10; let b01 = a00 * a12 - a02 * a10; let b02 = a00 * a13 - a03 * a10;
  let b03 = a01 * a12 - a02 * a11; let b04 = a01 * a13 - a03 * a11; let b05 = a02 * a13 - a03 * a12;
  let b06 = a20 * a31 - a21 * a30; let b07 = a20 * a32 - a22 * a30; let b08 = a20 * a33 - a23 * a30;
  let b09 = a21 * a32 - a22 * a31; let b10 = a21 * a33 - a23 * a31; let b11 = a22 * a33 - a23 * a32;
  let d = 1.0 / (b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06);
  return mat4x4<f32>(
    vec4<f32>((a11 * b11 - a12 * b10 + a13 * b09) * d, (a02 * b10 - a01 * b11 - a03 * b09) * d,
              (a31 * b05 - a32 * b04 + a33 * b03) * d, (a22 * b04 - a21 * b05 - a23 * b03) * d),
    vec4<f32>((a12 * b08 - a10 * b11 - a13 * b07) * d, (a00 * b11 - a02 * b08 + a03 * b07) * d,
              (a32 * b02 - a30 * b05 - a33 * b01) * d, (a20 * b05 - a22 * b02 + a23 * b01) * d),
    vec4<f32>((a10 * b10 - a11 * b08 + a13 * b06) * d, (a01 * b08 - a00 * b10 - a03 * b06) * d,
              (a30 * b04 - a31 * b02 + a33 * b00) * d, (a21 * b02 - a20 * b04 - a23 * b00) * d),
    vec4<f32>((a11 * b07 - a10 * b09 - a12 * b06) * d, (a00 * b09 - a01 * b07 + a02 * b06) * d,
              (a31 * b01 - a30 * b03 - a32 * b00) * d, (a20 * b03 - a21 * b01 + a22 * b00) * d));
}

fn put(vp: mat4x4<f32>, inv: mat4x4<f32>, eye: vec4<f32>, forward: vec3<f32>) {
  for (var c = 0u; c < 4u; c++) {
    head[c] = vp[c];
    head[4u + c] = inv[c];
  }
  head[8] = eye;
  head[9] = vec4<f32>(forward, 0.0);
}

@compute @workgroup_size(1) fn loft_camera() {
  let aspect = frame.viewport.x / frame.viewport.y;
  if (lens[0].x == 0.0) {
    let eye = lens[1].xyz;
    let to = lens[2].xyz - eye;
    let distance = length(to);
    let near = distance / 100.0;
    let far = distance * 100.0;
    let f = to / distance;
    let pole = dot(f, normalize(lens[3].xyz));
    let reference = select(lens[3].xyz, vec3<f32>(0.0, 0.0, -sign(pole)), abs(pole) > 0.999999);
    let r = normalize(cross(f, reference));
    let u = cross(r, f);
    let view = mat4x4<f32>(vec4<f32>(r.x, u.x, -f.x, 0.0), vec4<f32>(r.y, u.y, -f.y, 0.0), vec4<f32>(r.z, u.z, -f.z, 0.0),
                           vec4<f32>(-dot(r, eye), -dot(u, eye), dot(f, eye), 1.0));
    let s = 1.0 / tan(lens[0].y / 2.0);
    let proj = mat4x4<f32>(vec4<f32>(s / aspect, 0.0, 0.0, 0.0), vec4<f32>(0.0, s, 0.0, 0.0),
                           vec4<f32>(0.0, 0.0, far / (near - far), -1.0), vec4<f32>(0.0, 0.0, near * far / (near - far), 0.0));
    let vp = proj * view;
    put(vp, invert(vp), vec4<f32>(eye, 1.0), f);
  } else {
    // A matrix: the eye is where clip w is 0 (perspective), or the near plane's centre with w 0 (orthographic).
    let vp = mat4x4<f32>(lens[1], lens[2], lens[3], lens[4]);
    let inv = invert(vp);
    let centre = inv * vec4<f32>(0.0, 0.0, 1.0, 0.0);
    let n = inv * vec4<f32>(0.0, 0.0, 0.0, 1.0);
    let fr = inv * vec4<f32>(0.0, 0.0, 1.0, 1.0);
    let forward = normalize(fr.xyz / fr.w - n.xyz / n.w);
    let projective = abs(centre.w) > 1e-12 * max(length(centre.xyz), 1e-30);
    let eye = select(vec4<f32>(n.xyz / n.w, 0.0), vec4<f32>(centre.xyz / centre.w, 1.0), projective);
    put(vp, inv, eye, forward);
  }
}
