// Binary32 roundTiesToEven. Floating values cross the interface as opaque words.
// All NaN arithmetic returns the same quiet NaN. Bitcast itself preserves payloads.
const D_SIGN = 0x80000000u;
const D_INF = 0x7f800000u;
const D_NAN = 0x7fc00000u;
const D_FRAC = 0x007fffffu;
const D_HIDDEN = 0x00800000u;

struct DParts { m: u32, e: i32 }

fn d_parts(a: u32) -> DParts {
    let e = a >> 23u;
    if e != 0u { return DParts((a & D_FRAC) | D_HIDDEN, i32(e)); }
    let s = countLeadingZeros(a) - 8u;
    return DParts(a << s, 1 - i32(s));
}

fn d_jam(a: u32, by: u32) -> u32 {
    if by == 0u { return a; }
    if by >= 32u { return u32(a != 0u); }
    return (a >> by) | u32((a << (32u - by)) != 0u);
}

// m has its hidden bit at 26 and the bottom three bits are guard, round and sticky.
fn d_pack(s: u32, e: i32, m: u32) -> u32 {
    let shifted = d_jam(m, u32(max(1 - e, 0)));
    let low = shifted & 7u;
    let rounded = (shifted >> 3u) + u32(low > 4u || (low == 4u && (shifted & 8u) != 0u));
    if e >= 255 || (e == 254 && rounded == 0x01000000u) { return s | D_INF; }
    // Adding the significand lets a carry promote subnormals and powers of two.
    return s | ((u32(max(e, 1) - 1) << 23u) + rounded);
}

fn d_add_bits(x: u32, y: u32) -> u32 {
    let ax = x & ~D_SIGN;
    let ay = y & ~D_SIGN;
    let swap = ax < ay;
    let a = max(ax, ay);
    let b = min(ax, ay);
    let sign_a = select(x, y, swap) & D_SIGN;
    let sign_b = select(y, x, swap) & D_SIGN;
    if a >= D_INF {
        if a > D_INF || (b == D_INF && sign_a != sign_b) { return D_NAN; }
        return sign_a | D_INF;
    }
    if b == 0u {
        if a == 0u { return x & y; }
        return sign_a | a;
    }
    let p = d_parts(a);
    let q = d_parts(b);
    let lhs = p.m << 3u;
    let rhs = d_jam(q.m << 3u, u32(p.e - q.e));
    let m = select(lhs - rhs, lhs + rhs, sign_a == sign_b);
    if m == 0u { return 0u; }
    if m >= 0x08000000u { return d_pack(sign_a, p.e + 1, d_jam(m, 1u)); }
    let shift = countLeadingZeros(m) - 5u;
    return d_pack(sign_a, p.e - i32(shift), m << shift);
}

// Little-endian pairs of base-2^32 limbs.
fn d_product(a: u32, b: u32) -> vec2u {
    let low = (a & 0xffffu) * (b & 0xffffu);
    let mid = (a >> 16u) * (b & 0xffffu) + (b >> 16u) * (a & 0xffffu) + (low >> 16u);
    return vec2u((low & 0xffffu) | (mid << 16u), (a >> 16u) * (b >> 16u) + (mid >> 16u));
}

fn d_wide_jam(a: vec2u, by: u32) -> vec2u {
    if by == 0u { return a; }
    if by >= 32u { return vec2u(d_jam(a.y, by - 32u) | u32(a.x != 0u), 0u); }
    return vec2u((a.y << (32u - by)) | d_jam(a.x, by), a.y >> by);
}

fn d_wide_left(a: vec2u, by: u32) -> vec2u {
    if by == 0u { return a; }
    if by >= 32u { return vec2u(0u, a.x << (by - 32u)); }
    return vec2u(a.x << by, (a.y << by) | (a.x >> (32u - by)));
}

fn d_mul_bits(x: u32, y: u32) -> u32 {
    let a = x & ~D_SIGN;
    let b = y & ~D_SIGN;
    let s = (x ^ y) & D_SIGN;
    if a > D_INF || b > D_INF || (min(a, b) == 0u && max(a, b) == D_INF) { return D_NAN; }
    if max(a, b) == D_INF { return s | D_INF; }
    if min(a, b) == 0u { return s; }
    let p = d_parts(a);
    let q = d_parts(b);
    let prod = d_product(p.m, q.m);
    let carry = prod.y >> 15u;
    return d_pack(s, p.e + q.e - 127 + i32(carry), d_wide_jam(prod, 20u + carry).x);
}

fn d_fma_bits(x: u32, y: u32, z: u32) -> u32 {
    let a = x & ~D_SIGN;
    let b = y & ~D_SIGN;
    let c = z & ~D_SIGN;
    let s = (x ^ y) & D_SIGN;
    if max(a, b) >= D_INF || min(a, b) == 0u {
        return d_add_bits(d_mul_bits(x, y), z);
    }
    if c >= D_INF { return select(z, D_NAN, c > D_INF); }
    if c == 0u { return d_mul_bits(x, y); }
    let p = d_parts(a);
    let q = d_parts(b);
    let r = d_parts(c);
    let prod = d_product(p.m, q.m);
    let carry = prod.y >> 15u;
    let pe = p.e + q.e - 127 + i32(carry);
    let pm = d_wide_left(prod, 16u - carry);
    let rm = vec2u(0u, r.m << 7u);
    let swap = pe < r.e || (pe == r.e && (pm.y < rm.y || (pm.y == rm.y && pm.x < rm.x)));
    let e = max(pe, r.e);
    let lhs = select(pm, rm, swap);
    let rhs = d_wide_jam(select(rm, pm, swap), u32(abs(pe - r.e)));
    let sign_hi = select(s, z & D_SIGN, swap);
    if s == (z & D_SIGN) {
        let low = lhs.x + rhs.x;
        let high = lhs.y + rhs.y + u32(low < lhs.x);
        let up = high >> 31u;
        return d_pack(sign_hi, e + i32(up), d_wide_jam(vec2u(low, high), 36u + up).x);
    }
    let low = lhs.x - rhs.x;
    let high = lhs.y - rhs.y - u32(lhs.x < rhs.x);
    if (low | high) == 0u { return 0u; }
    let shift = select(countLeadingZeros(high) - 1u, countLeadingZeros(low) + 31u, high == 0u);
    return d_pack(sign_hi, e - i32(shift), d_wide_jam(d_wide_left(vec2u(low, high), shift), 36u).x);
}

fn d_div_bits(x: u32, y: u32) -> u32 {
    let a = x & ~D_SIGN;
    let b = y & ~D_SIGN;
    let s = (x ^ y) & D_SIGN;
    if a > D_INF || b > D_INF || (a == b && (a == D_INF || a == 0u)) { return D_NAN; }
    if a == D_INF || b == 0u { return s | D_INF; }
    if a == 0u || b == D_INF { return s; }
    let p = d_parts(a);
    let q = d_parts(b);
    let shift = u32(p.m < q.m);
    var rem = (p.m << shift) - q.m;
    var m = 1u;
    // Divide two 14-bit digits by the top 16 bits of the divisor.
    // Ignoring its low 8 bits overestimates by at most one, since
    // 2^14 * 255 < 2^23. The signed residual fits in 25 bits even
    // though the shifted dividend and product each need 38 bits.
    let divisor_hi = q.m >> 8u;
    for (var i = 0u; i < 2u; i++) {
        let guess = (rem << 6u) / divisor_hi;
        let delta = (rem << 14u) - guess * q.m;
        let over = bitcast<i32>(delta) < 0;
        rem = delta + select(0u, q.m, over);
        m = (m << 14u) | (guess - u32(over));
    }
    return d_pack(s, p.e - q.e + 127 - i32(shift), d_jam(m, 2u) | u32(rem != 0u));
}

fn d_sqrt_bits(x: u32) -> u32 {
    let a = x & ~D_SIGN;
    if a == 0u { return x; }
    if x != a || a > D_INF { return D_NAN; }
    if a == D_INF { return x; }
    let p = d_parts(a);
    let odd = (p.e - 127) & 1;
    let shift = 29u + u32(odd);
    let rad = d_wide_left(vec2u(p.m, 0u), shift);
    // The normalized radicand has 53 or 54 bits, so the first base-four
    // digit gives root 1. Each subsequent nibble extends the root by two
    // bits. Subtract (4*root + k)^2 - (4*root)^2 for the largest k that fits.
    var root = 1u;
    var rem = (rad.y >> 20u) - 1u;
    for (var i = 13u; i > 0u; i--) {
        let pos = (i - 1u) * 4u;
        let nibble = (select(rad.x, rad.y, pos >= 32u) >> (pos & 31u)) & 15u;
        let n = (rem << 4u) | nibble;
        let base = root << 3u;
        let k = u32(n >= base + 1u) + u32(n >= 2u * (base + 2u)) + u32(n >= 3u * (base + 3u));
        rem = n - k * (base + k);
        root = (root << 2u) | k;
    }
    return d_pack(0u, ((p.e - 127 - odd) / 2) + 127, root | u32(rem != 0u));
}

fn d_float(x: u32) -> f32 { return bitcast<f32>(x); }
fn d_bits(x: f32) -> u32 { return bitcast<u32>(x); }
fn d_add(x: f32, y: f32) -> f32 { return d_float(d_add_bits(d_bits(x), d_bits(y))); }
fn d_sub(x: f32, y: f32) -> f32 { return d_float(d_add_bits(d_bits(x), d_bits(y) ^ D_SIGN)); }
fn d_mul(x: f32, y: f32) -> f32 { return d_float(d_mul_bits(d_bits(x), d_bits(y))); }
fn d_div(x: f32, y: f32) -> f32 { return d_float(d_div_bits(d_bits(x), d_bits(y))); }
fn d_fma(x: f32, y: f32, z: f32) -> f32 { return d_float(d_fma_bits(d_bits(x), d_bits(y), d_bits(z))); }
fn d_sqrt(x: f32) -> f32 { return d_float(d_sqrt_bits(d_bits(x))); }
fn d_abs(x: f32) -> f32 { return d_float(d_bits(x) & ~D_SIGN); }
fn d_neg(x: f32) -> f32 { return d_float(d_bits(x) ^ D_SIGN); }

fn d_key(x: u32) -> u32 { return x ^ select(D_SIGN, 0xffffffffu, (x & D_SIGN) != 0u); }
fn d_nan(x: u32) -> bool { return (x & ~D_SIGN) > D_INF; }
fn d_eq(x: f32, y: f32) -> bool {
    let a = d_bits(x); let b = d_bits(y);
    return !d_nan(a) && (a == b || ((a | b) & ~D_SIGN) == 0u);
}
fn d_ne(x: f32, y: f32) -> bool { return !d_eq(x, y); }
fn d_lt(x: f32, y: f32) -> bool {
    let a = d_bits(x); let b = d_bits(y);
    return !d_nan(a) && !d_nan(b) && ((a | b) & ~D_SIGN) != 0u && d_key(a) < d_key(b);
}
fn d_le(x: f32, y: f32) -> bool { return d_lt(x, y) || d_eq(x, y); }
fn d_gt(x: f32, y: f32) -> bool { return d_lt(y, x); }
fn d_ge(x: f32, y: f32) -> bool { return d_le(y, x); }

fn d_min(x: f32, y: f32) -> f32 {
    let a = d_bits(x); let b = d_bits(y);
    if d_nan(a) || d_nan(b) { return d_float(D_NAN); }
    return d_float(select(b, a, d_key(a) < d_key(b)));
}
fn d_max(x: f32, y: f32) -> f32 {
    let a = d_bits(x); let b = d_bits(y);
    if d_nan(a) || d_nan(b) { return d_float(D_NAN); }
    return d_float(select(b, a, d_key(a) > d_key(b)));
}
fn d_clamp(x: f32, lo: f32, hi: f32) -> f32 { return d_min(d_max(x, lo), hi); }
fn d_sign(x: f32) -> f32 {
    let a = d_bits(x);
    if d_nan(a) { return d_float(D_NAN); }
    if (a & ~D_SIGN) == 0u { return x; }
    return d_float((a & D_SIGN) | 0x3f800000u);
}

fn d_floor(x: f32) -> f32 {
    let a = d_bits(x);
    let abs_a = a & ~D_SIGN;
    if d_nan(a) { return d_float(D_NAN); }
    if abs_a == 0u || abs_a >= 0x4b000000u { return x; }
    if abs_a < 0x3f800000u { return d_float(select(0u, 0xbf800000u, a != abs_a)); }
    let fraction = D_FRAC >> ((abs_a >> 23u) - 127u);
    if (a & fraction) == 0u { return x; }
    return d_float((a & ~fraction) + select(0u, fraction + 1u, a != abs_a));
}

fn d_fu(x: u32) -> f32 {
    if x == 0u { return d_float(0u); }
    let msb = 31u - countLeadingZeros(x);
    if msb <= 26u { return d_float(d_pack(0u, 127 + i32(msb), x << (26u - msb))); }
    return d_float(d_pack(0u, 127 + i32(msb), d_jam(x, msb - 26u)));
}
fn d_fi(x: i32) -> f32 {
    let a = bitcast<u32>(x);
    let mag = select(a, 0u - a, x < 0);
    return d_float(d_bits(d_fu(mag)) | (a & D_SIGN));
}
fn d_trunc_u(a: u32) -> u32 {
    if a < 0x3f800000u || a > D_INF { return 0u; }
    if a >= 0x4f800000u { return 0xffffffffu; }
    let p = d_parts(a);
    return select(p.m >> u32(max(150 - p.e, 0)), p.m << u32(max(p.e - 150, 0)), p.e >= 150);
}
fn d_uf(x: f32) -> u32 {
    let a = d_bits(x);
    return d_trunc_u(select(a, 0u, (a & D_SIGN) != 0u));
}
fn d_if(x: f32) -> i32 {
    let a = d_bits(x);
    let mag = d_trunc_u(a & ~D_SIGN);
    if (a & D_SIGN) == 0u { return i32(min(mag, 0x7fffffffu)); }
    return bitcast<i32>(0u - min(mag, 0x80000000u));
}

fn d_step(edge: f32, x: f32) -> f32 { return select(d_float(0u), d_float(0x3f800000u), d_ge(x, edge)); }
fn d_mix(x: f32, y: f32, t: f32) -> f32 {
    return d_add(d_mul(x, d_sub(d_float(0x3f800000u), t)), d_mul(y, t));
}
fn d_smoothstep(lo: f32, hi: f32, x: f32) -> f32 {
    let t = d_clamp(d_div(d_sub(x,lo),d_sub(hi,lo)), d_float(0u), d_float(0x3f800000u));
    return d_mul(d_mul(t,t),d_sub(d_float(0x40400000u),d_mul(d_float(0x40000000u),t)));
}
