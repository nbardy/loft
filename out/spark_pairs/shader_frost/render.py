"""CPU mirror renderer for out/spark_pairs/shader_frost/*.glsl.

Reimplements the exact shader math (hash22, 3x3 voronoi F1/F2, thin-film
palette, value-noise grain, lit-variant relief normals) in numpy so the GLSL
has runnable evidence without a GL context. Deterministic: all randomness is
hash-based, no RNG seed needed; TIME frozen per render via --time.

Outputs (default --size 512x288):
  render_base.png    shader.glsl @ t=0
  render_static.png  shader_static.glsl
  render_lit.png     shader_lit.glsl @ t=0
  vote_detail.png    detail end of the detail-vs-noise pair (= static params)
  vote_noise.png     noise end (same layout, extra jitter octave + heavy grain)
"""
import argparse
import os
import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))

# --- GLSL mirrors -----------------------------------------------------------

def fract(x):
    return x - np.floor(x)

def hash22(p):
    # p: (..., 2)
    q = fract(np.stack([p[..., 0], p[..., 1], p[..., 0]], axis=-1)
              * np.array([0.1031, 0.1030, 0.0973]))
    d = np.sum(q * (np.stack([q[..., 1], q[..., 2], q[..., 0]], axis=-1) + 33.33), axis=-1)
    q = q + d[..., None]
    return fract(np.stack([(q[..., 0] + q[..., 1]) * q[..., 2],
                           (q[..., 0] + q[..., 2]) * q[..., 1]], axis=-1))

def vnoise(p):
    i = np.floor(p)
    f = p - i
    u = f * f * (3.0 - 2.0 * f)
    def h(o):
        q = i + o
        return fract(np.sin(q[..., 0] * 127.1 + q[..., 1] * 311.7) * 43758.5453)
    a = h(np.array([0.0, 0.0])); b = h(np.array([1.0, 0.0]))
    c = h(np.array([0.0, 1.0])); d = h(np.array([1.0, 1.0]))
    return a + (b - a) * u[..., 0] + (c - a) * u[..., 1] + (a - b - c + d) * u[..., 0] * u[..., 1]

def voronoi(p):
    # p: (H, W, 2) in cell units -> f1, f2, cellHash
    ip = np.floor(p); fp = p - ip
    f1 = np.full(p.shape[:2], 8.0); f2 = np.full(p.shape[:2], 8.0)
    cid = np.zeros(p.shape[:2] + (2,))
    for j in (-1, 0, 1):
        for i in (-1, 0, 1):
            g = np.array([float(i), float(j)])
            o = hash22(ip + g)
            r = g + o - fp
            d = np.sum(r * r, axis=-1)
            m1 = d < f1
            f2 = np.where(m1, f1, np.minimum(f2, d))
            cid = np.where(m1[..., None], ip + g, cid)
            f1 = np.where(m1, d, f1)
    f1 = np.sqrt(f1); f2 = np.sqrt(f2)
    ch = fract(np.sin(cid[..., 0] * 127.1 + cid[..., 1] * 311.7) * 43758.5453)
    return f1, f2, ch

def thinfilm(t):
    t = t[..., None] * np.array([1.0, 1.15, 1.32]) + np.array([0.0, 0.33, 0.67])
    return 0.5 + 0.5 * np.cos(6.2831853 * t)

def smoothstep(e0, e1, x):
    t = np.clip((x - e0) / (e1 - e0), 0.0, 1.0)
    return t * t * (3.0 - 2.0 * t)

def coords(w, h):
    ys, xs = np.mgrid[0:h, 0:w]
    uv = np.stack([(xs + 0.5) / w, (ys + 0.5) / h], axis=-1)
    p = (uv - 0.5) * np.array([w / h, 1.0])
    pix = np.stack([xs + 0.5, ys + 0.5], axis=-1)
    return uv, p, pix

def render_base(w, h, cells=5.0, vein_w=0.09, irid=1.0, grain=0.6,
                t=0.0, jitter_extra=0.0, grain_boost=1.0):
    uv, p, pix = coords(w, h)
    wp = p * cells + 0.05 * np.stack(
        [vnoise(p * 3.0 + np.array([t * 0.05, 0.0])),
         vnoise(p * 3.0 + np.array([7.7, -t * 0.04]))], axis=-1)
    if jitter_extra > 0.0:
        wp = wp + jitter_extra * (np.stack(
            [vnoise(p * 14.0 + 3.7), vnoise(p * 14.0 + 9.2)], axis=-1) - 0.5)
    f1, f2, ch = voronoi(wp)
    edge = f2 - f1
    vein = 1.0 - smoothstep(0.0, vein_w, edge)
    iri = thinfilm(f1 * 1.4 + ch) * irid
    field = (np.array([0.62, 0.72, 0.82])[None, None, :]
             + (np.array([0.82, 0.89, 0.95]) - np.array([0.62, 0.72, 0.82]))[None, None, :] * vein[..., None])
    field = field + 0.10 * (f1 - 0.5)[..., None]
    g = vnoise(pix * 0.55 + t * 2.0) * vnoise(pix * 0.23 - t * 1.3)
    if grain_boost != 1.0:
        g = g + (grain_boost - 1.0) * 0.35 * vnoise(pix * 1.7 + 5.0)
    sparkle = (1.0 - smoothstep(0.0, 0.055, np.abs(g - 0.25))) * grain
    col = field + vein[..., None] * (0.25 + 0.75 * iri)
    col = col + sparkle[..., None] * (0.35 + 0.65 * vein[..., None])
    q = uv - 0.5
    col = col * (1.0 - 0.45 * np.sum(q * q, axis=-1))[..., None]
    return np.clip(col, 0.0, 1.0)

def relief_h(wp, vein_w):
    f1, f2, _ = voronoi(wp)
    edge = f2 - f1
    vein = 1.0 - smoothstep(0.0, vein_w, edge)
    return f1 * 0.55 + vein * 0.30, f1, f2, vein

def render_lit(w, h, cells=5.0, vein_w=0.09, t=0.0):
    uv, p, pix = coords(w, h)
    wp = p * cells + 0.05 * np.stack(
        [vnoise(p * 3.0 + np.array([t * 0.05, 0.0])),
         vnoise(p * 3.0 + np.array([7.7, -t * 0.04]))], axis=-1)
    _, f1, f2, vein = relief_h(wp, vein_w)
    iri = thinfilm(f1 * 1.4 + fract(f1 * 12.9) * 0.37)
    albedo = (np.array([0.55, 0.66, 0.78])[None, None, :]
              + (np.array([0.80, 0.88, 0.95]) - np.array([0.55, 0.66, 0.78]))[None, None, :] * vein[..., None])
    albedo = albedo + vein[..., None] * 0.55 * iri
    e = 0.012
    h0, _, _, _ = relief_h(wp, vein_w)
    hx, _, _, _ = relief_h(wp + np.array([e, 0.0]), vein_w)
    hy, _, _, _ = relief_h(wp + np.array([0.0, e]), vein_w)
    bump = 2.2
    nx = -(hx - h0) * bump / e * 0.12
    ny = -(hy - h0) * bump / e * 0.12
    inv = 1.0 / np.sqrt(nx * nx + ny * ny + 1.0)
    n = np.stack([nx * inv, ny * inv, inv], axis=-1)
    key = np.array([-0.55, 0.62, 0.55]); key = key / np.linalg.norm(key)
    fill = np.array([0.6, -0.25, 0.45]); fill = fill / np.linalg.norm(fill)
    vv = np.array([0.0, 0.0, 1.0])
    dif = np.maximum(np.sum(n * key, axis=-1), 0.0)
    fil = np.maximum(np.sum(n * fill, axis=-1), 0.0)
    hh = (key + vv) / np.linalg.norm(key + vv)
    spec = np.power(np.maximum(np.sum(n * hh, axis=-1), 0.0), 60.0) * (0.35 + 0.65 * vein)
    col = albedo * (0.38 + 0.72 * dif[..., None] * np.array([0.85, 0.93, 1.0])
                    + 0.25 * fil[..., None] * np.array([1.0, 0.85, 0.7]))
    col = col + spec[..., None] * np.array([0.9, 0.96, 1.0])
    q = uv - 0.5
    col = col * (1.0 - 0.45 * np.sum(q * q, axis=-1))[..., None]
    return np.clip(col, 0.0, 1.0)

def save(col, path):
    img = Image.fromarray(np.clip(col * 255.0 + 0.5, 0, 255).astype(np.uint8))
    img.save(path)
    return path

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--size", default="512x288")
    ap.add_argument("--time", type=float, default=0.0)
    args = ap.parse_args()
    w, h = map(int, args.size.split("x"))
    t = args.time
    outs = {}
    outs["render_base.png"] = render_base(w, h, t=t)
    outs["render_static.png"] = render_base(w, h, cells=7.5, vein_w=0.055, grain=0.35, t=0.0)
    outs["render_lit.png"] = render_lit(w, h, t=t)
    # Vote pair: content held fixed (same cells/time), only detail-vs-noise axis.
    outs["vote_detail.png"] = render_base(w, h, cells=7.5, vein_w=0.055, grain=0.35, t=0.0)
    outs["vote_noise.png"] = render_base(w, h, cells=7.5, vein_w=0.055, grain=1.6,
                                         t=0.0, jitter_extra=0.35, grain_boost=2.2)
    for name, col in outs.items():
        path = os.path.join(HERE, name)
        save(col, path)
        mean = col.mean(axis=(0, 1))
        print(f"{name} {w}x{h} mean_rgb=[{mean[0]:.3f},{mean[1]:.3f},{mean[2]:.3f}]")

if __name__ == "__main__":
    main()
