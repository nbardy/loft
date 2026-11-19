//! ECMAScript number spelling and the three WGSL literal printers.

use std::fmt::Write;

struct Decimal {
    digits: String,
    /// The power of ten of the leading digit.
    exp: i32,
}

impl Decimal {
    /// A binary64 is m * 2^e. For e < 0, m * 5^(-e) supplies the exact decimal
    /// digits. Keeping the exact tail distinguishes a decimal tie from a nearby value.
    fn exact(x: f64) -> Self {
        let bits = x.to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as i32;
        let mantissa = (bits & ((1 << 52) - 1)) | if exponent == 0 { 0 } else { 1 << 52 };
        let e = exponent.max(1) - 1075;
        if mantissa == 0 {
            return Self { digits: "0".into(), exp: 0 };
        }
        let zeros = (mantissa.trailing_zeros() as i32).min((-e).max(0));
        let mantissa = mantissa >> zeros;
        let e = e + zeros;
        if e == 0 {
            let digits = mantissa.to_string();
            return Self { exp: digits.len() as i32 - 1, digits };
        }
        const BASE: u64 = 1_000_000_000;
        let mut words = vec![(mantissa % BASE) as u32, (mantissa / BASE) as u32];
        let (factor, batch) = if e < 0 { (5_u64, 12) } else { (2_u64, 29) };
        let mut left = e.unsigned_abs();
        while left != 0 {
            let n = left.min(batch);
            let power = factor.pow(n);
            let mut carry = 0;
            for word in &mut words {
                let scaled = u64::from(*word) * power + carry;
                *word = (scaled % BASE) as u32;
                carry = scaled / BASE;
            }
            if carry != 0 {
                words.push(carry as u32);
            }
            left -= n;
        }
        if words.last() == Some(&0) {
            words.pop();
        }
        let mut digits = words.pop().expect("one or more decimal words").to_string();
        for word in words.iter().rev() {
            write!(digits, "{word:09}").expect("writing to a String");
        }
        Self { exp: digits.len() as i32 - 1 + e.min(0), digits }
    }

    /// Integer coefficient at this precision, rounded to nearest. Number#toString
    /// chooses an even coefficient at a tie. toPrecision and toFixed choose the larger.
    fn coefficient(&self, count: i32, ties_even: bool) -> String {
        if count < 0 {
            return "0".into();
        }
        let n = count as usize;
        let digits = self.digits.as_bytes();
        let mut out = digits[..n.min(digits.len())].to_vec();
        if n > digits.len() {
            out.resize(n, b'0');
        }
        let up = digits.get(n).is_some_and(|&d| {
            d > b'5'
                || (d == b'5'
                    && (!ties_even
                        || digits[n + 1..].iter().any(|&d| d != b'0')
                        || out.last().is_some_and(|d| d % 2 != 0)))
        });
        if up {
            for d in out.iter_mut().rev() {
                if *d != b'9' {
                    *d += 1;
                    return String::from_utf8(out).expect("decimal digits");
                }
                *d = b'0';
            }
            out.insert(0, b'1');
        }
        if out.is_empty() {
            out.push(b'0');
        }
        String::from_utf8(out).expect("decimal digits")
    }

    fn precision(&self, p: u32, negative: bool) -> String {
        let mut digits = self.coefficient(p as i32, false);
        let mut exp = self.exp;
        if digits.len() > p as usize {
            digits.pop();
            exp += 1;
        }
        spell(&digits, exp, negative, exp < -6 || exp >= p as i32)
    }

    fn shortest(&self, x: f64) -> (String, i32) {
        for p in 1..=17 {
            let coefficient = self.coefficient(p, true);
            let power = self.exp + 1 - p;
            let n = coefficient.parse::<u64>().expect("at most 18 digits");
            // At an exact power of two the two sides of the rounding interval
            // can have different widths. Try the adjacent coefficients as well.
            for candidate in [n, n - 1, n + 1] {
                let digits = candidate.to_string();
                if format!("{digits}e{power}").parse::<f64>() == Ok(x) {
                    let exp = digits.len() as i32 - 1 + power;
                    return (digits.trim_end_matches('0').to_owned(), exp);
                }
            }
        }
        unreachable!("17 significant digits round-trip binary64")
    }
}

fn spell(digits: &str, exp: i32, negative: bool, scientific: bool) -> String {
    let mut out = if negative { String::from("-") } else { String::new() };
    if scientific {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        write!(out, "e{exp:+}").expect("writing to a String");
    } else if exp < 0 {
        out.push_str("0.");
        out.extend(std::iter::repeat_n('0', (-1 - exp) as usize));
        out.push_str(digits);
    } else {
        let point = (exp + 1) as usize;
        if point < digits.len() {
            out.push_str(&digits[..point]);
            out.push('.');
            out.push_str(&digits[point..]);
        } else {
            out.push_str(digits);
            out.extend(std::iter::repeat_n('0', point - digits.len()));
        }
    }
    out
}

/// JavaScript String(number), in radix ten.
pub fn to_string(x: f64) -> String {
    if x == 0.0 {
        return "0".into();
    }
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x < 0.0 { "-Infinity" } else { "Infinity" }.into();
    }
    let (digits, exp) = Decimal::exact(x).shortest(x.abs());
    spell(&digits, exp, x < 0.0, !(-6..21).contains(&exp))
}

/// JavaScript Number#toFixed with a digit count in 0..=100.
pub fn to_fixed(x: f64, digits: u32) -> String {
    assert!(digits <= 100);
    if !x.is_finite() || x.abs() >= 1e21 {
        return to_string(x);
    }
    let exact = Decimal::exact(x);
    let mut fixed = exact.coefficient(exact.exp + 1 + digits as i32, false);
    let digits = digits as usize;
    if fixed.len() <= digits {
        fixed.insert_str(0, &"0".repeat(digits + 1 - fixed.len()));
    }
    if digits != 0 {
        fixed.insert(fixed.len() - digits, '.');
    }
    if x < 0.0 {
        fixed.insert(0, '-');
    }
    fixed
}

/// JavaScript Number#toPrecision with a precision in 1..=100.
pub fn to_precision(x: f64, precision: u32) -> String {
    if !x.is_finite() {
        return to_string(x);
    }
    assert!((1..=100).contains(&precision));
    Decimal::exact(x).precision(precision, x < 0.0)
}

fn is_integer(x: f64) -> bool {
    x.fract() == 0.0
}

/// expr.ts: the shortest toPrecision that WGSL reads as the same f32.
pub fn lit(n: f64) -> String {
    if is_integer(n) {
        return to_fixed(n, 1);
    }
    let f = n as f32;
    if n.is_finite() {
        let decimal = Decimal::exact(n);
        for p in 1..17 {
            let s = decimal.precision(p, n < 0.0);
            let rounded = s.parse::<f64>().expect("a finite decimal");
            if rounded as f32 == f {
                return to_string(rounded);
            }
        }
    }
    to_string(n)
}

/// scene_wgsl.ts: String(x), with ".0" appended when x is an integer.
pub fn scene_lit(x: f64) -> String {
    let mut text = to_string(x);
    if is_integer(x) {
        text.push_str(".0");
    }
    text
}

/// recur.ts: integers use toFixed(1), other numbers fround before spelling.
pub fn recur_lit(x: f64) -> String {
    if is_integer(x) { to_fixed(x, 1) } else { to_string(f64::from(x as f32)) }
}
