//! Lane SW. src/host/files.ts writeBundle, src/host/lower.ts chunkName, and JSON spelled as JSON.stringify.

use std::cmp::Ordering;
use std::fmt::{self, Write as _};
use std::fs;
use std::io;
use std::path::Path;

use serde::Deserialize;
use serde::ser::{self, Serialize, Serializer};

use crate::Error;

const MANIFEST: &str = "manifest.json";

/// Two FNV-1a hashes of the UTF-16 code units, in hex. Only names within one artefact must differ.
pub fn chunk_name(text: &str) -> String {
    let (mut a, mut b) = (0x811c9dc5u32, 0x050c5d1fu32);
    for c in text.encode_utf16().map(u32::from) {
        a = (a ^ c).wrapping_mul(0x01000193);
        b = (b ^ c).wrapping_mul(0x01000193) ^ (b >> 15);
    }
    format!("{a:08x}{b:08x}")
}

/// Replaces the files an earlier build wrote. Anything else in the directory is refused.
pub fn write_bundle(
    dir: &Path,
    manifest_json: &str,
    chunks: &[(String, String)],
    blobs: &[(String, Vec<u8>)],
) -> Result<(), Error> {
    #[derive(Deserialize)]
    struct Manifest {
        tables: Vec<Table>,
    }

    #[derive(Deserialize)]
    struct Table {
        name: String,
        fill: Fill,
    }

    #[derive(Deserialize)]
    #[serde(tag = "kind")]
    enum Fill {
        #[serde(rename = "cpu")]
        Cpu { why: String },
        #[serde(other)]
        Other,
    }

    // A cpu table is a JavaScript fill: refused by name, before anything is written.
    let manifest: Manifest =
        serde_json::from_str(manifest_json).map_err(|e| Error::Internal(format!("build: manifest: {e}")))?;
    let cpu: Vec<_> = manifest
        .tables
        .into_iter()
        .filter_map(|t| match t.fill {
            Fill::Cpu { why } => Some(format!("\"{}\" ({why})", t.name)),
            Fill::Other => None,
        })
        .collect();
    if !cpu.is_empty() {
        return Err(Error::Scene(format!(
            "build: {} table(s) are filled by JavaScript, which a file cannot hold: {}",
            cpu.len(),
            cpu.join(", "),
        )));
    }

    let write = || -> io::Result<Result<(), Error>> {
        fs::create_dir_all(dir)?;
        let mut stray = Vec::new();
        for entry in fs::read_dir(dir)? {
            let name = entry?.file_name();
            if name != MANIFEST && name != "wgsl" && name != "bin" {
                stray.push(name.to_string_lossy().into_owned());
            }
        }
        if !stray.is_empty() {
            return Ok(Err(Error::Internal(format!(
                "build: {} holds {}, which a build did not write; give an empty or earlier build directory",
                dir.display(),
                stray.join(", "),
            ))));
        }
        let wgsl = dir.join("wgsl");
        let bin = dir.join("bin");
        for path in [&wgsl, &bin] {
            match fs::symlink_metadata(path) {
                Ok(info) if info.is_dir() => fs::remove_dir_all(path)?,
                Ok(_) => fs::remove_file(path)?,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
        fs::create_dir(&wgsl)?;
        fs::create_dir(&bin)?;
        for (name, text) in chunks {
            fs::write(wgsl.join(format!("{name}.wgsl")), text)?;
        }
        for (name, bytes) in blobs {
            fs::write(bin.join(format!("{name}.bin")), bytes)?;
        }
        fs::write(dir.join(MANIFEST), manifest_json)?;
        Ok(Ok(()))
    };
    write().map_err(|e| Error::Internal(format!("build: {}: {e}", dir.display())))?
}

/// Compact JSON for a serializable value whose numbers are finite.
pub fn to_json<T: Serialize>(value: &T) -> String {
    try_to_json(value).unwrap_or_else(|e| panic!("{e}"))
}

/// Fallible form for a manifest whose numbers come from the scene.
pub fn try_to_json<T: Serialize>(value: &T) -> Result<String, Error> {
    value.serialize(Json { key: "", utf16: false }).map_err(|e: JsonError| Error::Internal(e.0))
}

#[derive(Debug)]
struct JsonError(String);

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for JsonError {}

impl ser::Error for JsonError {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self(message.to_string())
    }
}

/// A JavaScript string from UTF-16 code units, including any unpaired surrogates.
pub struct JsString<'a>(pub &'a [u16]);

const UTF16: &str = "loft::JsString";

impl Serialize for JsString<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        struct Units<'a>(&'a [u16]);
        impl Serialize for Units<'_> {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let bytes: Vec<_> = self.0.iter().flat_map(|u| u.to_le_bytes()).collect();
                serializer.serialize_bytes(&bytes)
            }
        }
        serializer.serialize_newtype_struct(UTF16, &Units(self.0))
    }
}

/// QuoteJSONString operates on code points and escapes any unpaired UTF-16 code unit.
fn quote_json(chars: impl Iterator<Item = Result<char, u16>>) -> String {
    let mut out = String::from("\"");
    for c in chars {
        match c {
            Ok('"') => out.push_str("\\\""),
            Ok('\\') => out.push_str("\\\\"),
            Ok('\u{8}') => out.push_str("\\b"),
            Ok('\u{c}') => out.push_str("\\f"),
            Ok('\n') => out.push_str("\\n"),
            Ok('\r') => out.push_str("\\r"),
            Ok('\t') => out.push_str("\\t"),
            Ok(c @ '\0'..='\u{1f}') => write!(out, "\\u{:04x}", u32::from(c)).unwrap(),
            Ok(c) => out.push(c),
            Err(unit) => write!(out, "\\u{unit:04x}").unwrap(),
        }
    }
    out.push('"');
    out
}

type JsonResult<T = String> = Result<T, JsonError>;

#[derive(Clone, Copy)]
struct Json<'a> {
    key: &'a str,
    utf16: bool,
}

// Integer types in the manifest represent ECMAScript Numbers too. This also handles a value
// outside the safe-integer range the same way as the conversion to a Number on the TS side.
macro_rules! numbers {
    ($($method:ident: $ty:ty),+ $(,)?) => {
        $(fn $method(self, value: $ty) -> JsonResult {
            self.serialize_f64(value as f64)
        })+
    };
}

impl Serializer for Json<'_> {
    type Ok = String;
    type Error = JsonError;
    type SerializeSeq = Array;
    type SerializeTuple = Array;
    type SerializeTupleStruct = Array;
    type SerializeTupleVariant = Array;
    type SerializeMap = Object;
    type SerializeStruct = Object;
    type SerializeStructVariant = Object;

    numbers!(
        serialize_i8: i8, serialize_i16: i16, serialize_i32: i32, serialize_i64: i64, serialize_i128: i128,
        serialize_u8: u8, serialize_u16: u16, serialize_u32: u32, serialize_u64: u64, serialize_u128: u128,
        serialize_f32: f32,
    );

    fn serialize_f64(self, value: f64) -> JsonResult {
        // JSON.stringify normally writes NaN and Infinity as null. The bundle writer's replacer refuses them.
        // Finite doubles round-trip exactly through JSON.stringify and JSON.parse.
        if !value.is_finite() {
            let name = if value.is_nan() {
                "NaN"
            } else if value.is_sign_negative() {
                "-Infinity"
            } else {
                "Infinity"
            };
            return Err(JsonError(format!("build: manifest field \"{}\" is {name}, which JSON cannot hold", self.key)));
        }
        Ok(number_string(value))
    }

    fn serialize_bool(self, value: bool) -> JsonResult {
        Ok(value.to_string())
    }

    fn serialize_char(self, value: char) -> JsonResult {
        Ok(quote_json(std::iter::once(Ok(value))))
    }

    fn serialize_str(self, value: &str) -> JsonResult {
        Ok(quote_json(value.chars().map(Ok)))
    }

    fn serialize_bytes(self, value: &[u8]) -> JsonResult {
        if self.utf16 {
            let (pairs, remainder) = value.as_chunks::<2>();
            if !remainder.is_empty() {
                return Err(JsonError("an incomplete UTF-16 code unit".into()));
            }
            let units = pairs.iter().copied().map(u16::from_le_bytes);
            return Ok(quote_json(char::decode_utf16(units).map(|c| c.map_err(|e| e.unpaired_surrogate()))));
        }
        value.serialize(self)
    }

    fn serialize_none(self) -> JsonResult {
        self.serialize_unit()
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> JsonResult {
        value.serialize(self)
    }

    fn serialize_unit(self) -> JsonResult {
        Ok("null".into())
    }

    fn serialize_unit_struct(self, _: &'static str) -> JsonResult {
        self.serialize_unit()
    }

    fn serialize_unit_variant(self, _: &'static str, _: u32, variant: &'static str) -> JsonResult {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(mut self, name: &'static str, value: &T) -> JsonResult {
        self.utf16 = name == UTF16;
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        value: &T,
    ) -> JsonResult {
        let mut object = Object::new(1, None);
        object.field(variant, value)?;
        Ok(object.finish())
    }

    fn serialize_seq(self, _: Option<usize>) -> JsonResult<Array> {
        Ok(Array::new(None))
    }

    fn serialize_tuple(self, _: usize) -> JsonResult<Array> {
        self.serialize_seq(None)
    }

    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> JsonResult<Array> {
        self.serialize_seq(None)
    }

    fn serialize_tuple_variant(self, _: &'static str, _: u32, variant: &'static str, _: usize) -> JsonResult<Array> {
        Ok(Array::new(Some(variant)))
    }

    fn serialize_map(self, len: Option<usize>) -> JsonResult<Object> {
        Ok(Object::new(len.unwrap_or(0), None))
    }

    fn serialize_struct(self, _: &'static str, len: usize) -> JsonResult<Object> {
        Ok(Object::new(len, None))
    }

    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        len: usize,
    ) -> JsonResult<Object> {
        Ok(Object::new(len, Some(variant)))
    }
}

fn wrap_variant(variant: Option<&str>, value: String) -> String {
    match variant {
        Some(variant) => format!("{{{}:{value}}}", quote_json(variant.chars().map(Ok))),
        None => value,
    }
}

struct Array {
    out: String,
    length: usize,
    variant: Option<&'static str>,
}

impl Array {
    fn new(variant: Option<&'static str>) -> Self {
        Self { out: "[".into(), length: 0, variant }
    }

    fn push<T: ?Sized + Serialize>(&mut self, value: &T) -> JsonResult<()> {
        if self.length > 0 {
            self.out.push(',');
        }
        self.out.push_str(&value.serialize(Json { key: &self.length.to_string(), utf16: false })?);
        self.length += 1;
        Ok(())
    }

    fn finish(mut self) -> String {
        self.out.push(']');
        wrap_variant(self.variant, self.out)
    }
}

macro_rules! array {
    ($trait:ident::$method:ident) => {
        impl ser::$trait for Array {
            type Ok = String;
            type Error = JsonError;

            fn $method<T: ?Sized + Serialize>(&mut self, value: &T) -> JsonResult<()> {
                self.push(value)
            }

            fn end(self) -> JsonResult {
                Ok(self.finish())
            }
        }
    };
}

array!(SerializeSeq::serialize_element);
array!(SerializeTuple::serialize_element);
array!(SerializeTupleStruct::serialize_field);
array!(SerializeTupleVariant::serialize_field);

struct Object {
    entries: Vec<(String, String)>,
    pending: Option<String>,
    variant: Option<&'static str>,
}

impl Object {
    fn new(len: usize, variant: Option<&'static str>) -> Self {
        Self { entries: Vec::with_capacity(len), pending: None, variant }
    }

    fn field<T: ?Sized + Serialize>(&mut self, key: &str, value: &T) -> JsonResult<()> {
        let value = value.serialize(Json { key, utf16: false })?;
        self.insert(quote_json(key.chars().map(Ok)), value);
        Ok(())
    }

    fn insert(&mut self, key: String, value: String) {
        if let Some((_, previous)) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            *previous = value;
        } else {
            self.entries.push((key, value));
        }
    }

    fn finish(mut self) -> String {
        // OrdinaryOwnPropertyKeys puts array indices first, then the other keys in insertion order.
        // A stable sort keeps the order from the producer's struct or map.
        self.entries.sort_by_key(|(quoted, _)| {
            let k = &quoted[1..quoted.len() - 1];
            if k.len() > 1 && k.starts_with('0') || !k.bytes().all(|c| c.is_ascii_digit()) {
                return u32::MAX;
            }
            k.parse::<u32>().unwrap_or(u32::MAX)
        });
        let mut out = String::from("{");
        for (k, value) in self.entries {
            if out.len() > 1 {
                out.push(',');
            }
            out.push_str(&k);
            out.push(':');
            out.push_str(&value);
        }
        out.push('}');
        wrap_variant(self.variant, out)
    }
}

impl ser::SerializeMap for Object {
    type Ok = String;
    type Error = JsonError;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> JsonResult<()> {
        let key = key.serialize(Json { key: "", utf16: false })?;
        self.pending = Some(match key.as_bytes().first() {
            Some(b'"') => key,
            Some(b'{' | b'[') => return Err(JsonError("a JSON object key must be a string or scalar".into())),
            _ => quote_json(key.chars().map(Ok)),
        });
        Ok(())
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> JsonResult<()> {
        let key = self.pending.take().ok_or_else(|| JsonError("a JSON object value has no key".into()))?;
        let key_text = serde_json::from_str::<String>(&key);
        // The escaped spelling is available even for a key with an unpaired surrogate.
        let name = key_text.as_deref().unwrap_or(&key[1..key.len() - 1]);
        let value = value.serialize(Json { key: name, utf16: false })?;
        self.insert(key, value);
        Ok(())
    }

    fn end(self) -> JsonResult {
        Ok(self.finish())
    }
}

macro_rules! object {
    ($trait:ident) => {
        impl ser::$trait for Object {
            type Ok = String;
            type Error = JsonError;

            fn serialize_field<T: ?Sized + Serialize>(&mut self, key: &'static str, value: &T) -> JsonResult<()> {
                self.field(key, value)
            }

            fn end(self) -> JsonResult {
                Ok(self.finish())
            }
        }
    };
}

object!(SerializeStruct);
object!(SerializeStructVariant);

/// A fixed integer for the exact binary64 rounding interval. Eighteen limbs hold 2^1076 (the interval divisor
/// for subnormals), with room for an additional decimal digit. Limbs run from least to most significant.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Int([u64; 18]);

impl Int {
    fn shifted(value: u64, shift: i32) -> Self {
        let mut limbs = [0; 18];
        let limb = shift as usize / 64;
        let bits = shift as u32 % 64;
        limbs[limb] = value << bits;
        if bits != 0 {
            limbs[limb + 1] = value >> (64 - bits);
        }
        Self(limbs)
    }

    fn cmp(&self, other: &Self) -> Ordering {
        self.0.iter().rev().cmp(other.0.iter().rev())
    }

    fn mul(&mut self, by: u64) {
        let mut carry = 0;
        for limb in &mut self.0 {
            let product = u128::from(*limb) * u128::from(by) + carry;
            *limb = product as u64;
            carry = product >> 64;
        }
        debug_assert_eq!(carry, 0);
    }

    fn mul_pow10(&mut self, mut exp: u32) {
        while exp >= 19 {
            self.mul(10u64.pow(19));
            exp -= 19;
        }
        self.mul(10u64.pow(exp));
    }

    fn sub(&mut self, other: &Self) {
        let mut borrow = false;
        for (x, y) in self.0.iter_mut().zip(other.0) {
            let (difference, first) = x.overflowing_sub(y);
            let (difference, second) = difference.overflowing_sub(u64::from(borrow));
            *x = difference;
            borrow = first || second;
        }
        debug_assert!(!borrow);
    }
}

/// Number::toString, base ten. Work with the exact value and both rounding midpoints, then stop at the first
/// decimal digit where either rounding direction round-trips. The midpoint below a power of two can be closer.
/// See https://tc39.es/ecma262/multipage/ecmascript-data-types-and-values.html#sec-numeric-types-number-tostring
pub(crate) fn number_string(value: f64) -> String {
    let x = value.abs();
    if x <= (1u64 << 53) as f64 && x == (x as u64) as f64 {
        return if value < 0.0 { format!("-{}", x as u64) } else { (x as u64).to_string() };
    }
    let bits = x.to_bits();
    let biased = (bits >> 52) as i32;
    let fraction = bits & ((1u64 << 52) - 1);
    let mantissa = fraction + if biased > 0 { 1u64 << 52 } else { 0 };
    let exp2 = biased.max(1) - 1023 - 52 - 2;
    let shift = exp2.max(0);
    let mut scaled = Int::shifted(mantissa * 4, shift);
    let mut below = Int::shifted(if fraction == 0 && biased > 1 { 1 } else { 2 }, shift);
    let mut above = Int::shifted(2, shift);
    let mut divisor = Int::shifted(1, (-exp2).max(0));
    let even = mantissa.is_multiple_of(2);
    // The approximate exponent only chooses a scale. The integer comparisons below fix it at powers of ten.
    let mut exp10 = x.log10().floor() as i32;
    if exp10 < 0 {
        let exp = (-exp10) as u32;
        scaled.mul_pow10(exp);
        below.mul_pow10(exp);
        above.mul_pow10(exp);
    } else {
        divisor.mul_pow10(exp10 as u32);
    }
    while scaled.cmp(&divisor).is_lt() {
        scaled.mul(10);
        below.mul(10);
        above.mul(10);
        exp10 -= 1;
    }
    let mut ten = divisor;
    ten.mul(10);
    while scaled.cmp(&ten).is_ge() {
        divisor = ten;
        ten.mul(10);
        exp10 += 1;
    }

    let mut digits = 0u64;
    loop {
        let mut digit = 0;
        while scaled.cmp(&divisor).is_ge() {
            scaled.sub(&divisor);
            digit += 1;
        }
        digits = digits * 10 + digit;
        let mut to_upper = divisor;
        to_upper.sub(&scaled);
        let low = scaled.cmp(&below);
        let high = to_upper.cmp(&above);
        let low = low.is_lt() || (low.is_eq() && even);
        let high = high.is_lt() || (high.is_eq() && even);
        if low || high {
            let nearest = scaled.cmp(&to_upper);
            if high && (!low || nearest.is_gt() || (nearest.is_eq() && !digits.is_multiple_of(2))) {
                digits += 1;
                if digits == 10u64.pow(digits.ilog10()) {
                    exp10 += 1;
                }
            }
            break;
        }
        scaled.mul(10);
        below.mul(10);
        above.mul(10);
    }

    let digits = digits.to_string();
    let digits = digits.trim_end_matches('0');
    let mut out = String::new();
    if value < 0.0 {
        out.push('-');
    }
    match exp10 {
        -6..=-1 => {
            out.push_str("0.");
            out.extend(std::iter::repeat_n('0', (-exp10 - 1) as usize));
            out.push_str(digits);
        }
        0..=20 => {
            let point = (exp10 + 1) as usize;
            if digits.len() <= point {
                out.push_str(digits);
                out.extend(std::iter::repeat_n('0', point - digits.len()));
            } else {
                out.push_str(&digits[..point]);
                out.push('.');
                out.push_str(&digits[point..]);
            }
        }
        _ => {
            out.push_str(&digits[..1]);
            if digits.len() > 1 {
                out.push('.');
                out.push_str(&digits[1..]);
            }
            write!(out, "e{exp10:+}").unwrap();
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde::de::IgnoredAny;
    use serde::ser::{SerializeMap, SerializeSeq};
    use std::path::PathBuf;

    const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../..");

    pub(crate) fn references() -> Vec<PathBuf> {
        let requested = std::env::var("LOFT_SW_TEST_RUN").ok();
        let run = requested.as_deref().unwrap_or("sw");
        let dir = Path::new(ROOT).join("out/port").join(run).join("entries");
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound && requested.is_none() => return Vec::new(),
            Err(e) => panic!("TS references for {run}: {e}"),
        };
        let paths: Vec<_> =
            entries.map(|e| e.unwrap().path().join("ts/artefact")).filter(|dir| dir.join(MANIFEST).is_file()).collect();
        assert!(requested.is_none() || !paths.is_empty(), "run {run} emitted no TS artefacts");
        paths
    }

    // serde_json's stream parser finds the value boundaries. Read the original object order and parse the original
    // number token as f64: Value sorts object keys and the default serde_json reader can round a decimal differently.
    struct Ordered<'a>(&'a str);

    fn split_json(text: &str) -> (&str, &str) {
        let mut stream = serde_json::Deserializer::from_str(text).into_iter::<IgnoredAny>();
        stream.next().unwrap().unwrap();
        text.split_at(stream.byte_offset())
    }

    impl Serialize for Ordered<'_> {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let raw = self.0;
            match raw.as_bytes()[0] {
                b'{' => {
                    let mut map = serializer.serialize_map(None)?;
                    let mut rest = &raw[1..];
                    while !rest.starts_with('}') {
                        // A top-level string in serde_json's stream must end at trivia or valid JSON punctuation
                        // other than ':'. Read the property name through the Deserializer instead.
                        let end = rest[1..]
                            .char_indices()
                            .scan(false, |escaped, (i, c)| {
                                let closing = c == '"' && !*escaped;
                                *escaped = c == '\\' && !*escaped;
                                Some(closing.then_some(i + 2))
                            })
                            .flatten()
                            .next()
                            .unwrap();
                        let key: String = serde_json::from_str(&rest[..end]).unwrap();
                        let (value, tail) = split_json(rest[end..].strip_prefix(':').unwrap());
                        map.serialize_entry(&key, &Ordered(value))?;
                        rest = tail.strip_prefix(',').unwrap_or(tail);
                    }
                    map.end()
                }
                b'[' => {
                    let mut seq = serializer.serialize_seq(None)?;
                    let mut rest = &raw[1..];
                    while !rest.starts_with(']') {
                        let (value, tail) = split_json(rest);
                        seq.serialize_element(&Ordered(value))?;
                        rest = tail.strip_prefix(',').unwrap_or(tail);
                    }
                    seq.end()
                }
                b'-' | b'0'..=b'9' => serializer.serialize_f64(raw.parse().unwrap()),
                _ => serde_json::from_str::<serde_json::Value>(raw).unwrap().serialize(serializer),
            }
        }
    }

    #[test]
    fn manifest_json_matches_ts_bytes() {
        // Catches sorted fields, Rust decimal/exponent spellings, and an appended newline.
        let numbers = [
            -0.0,
            1e-6,
            1e-7,
            1e20,
            1e21,
            1e23,
            1_000_000_000_000_000_128.0,
            f64::from_bits(1),
            f64::MAX,
            2f64.powi(-24),
            f64::MIN_POSITIVE,
        ];
        assert_eq!(
            to_json(&numbers),
            "[0,0.000001,1e-7,100000000000000000000,1e+21,1e+23,1000000000000000100,5e-324,1.7976931348623157e+308,5.960464477539063e-8,2.2250738585072014e-308]",
        );
        assert_eq!(
            to_json(&JsString(&[0xd83d, b'X' as u16, 0xdc00, 0xd83d, 0xde00, 0x2028, 0x1f])),
            "\"\\ud83dX\\udc00😀\u{2028}\\u001f\""
        );
        let ordered = r#"{"2":42,"10":43,"z":[1e-7,null],"01":50,"4294967295":51,"a":"a\u0000\n\\\""}"#;
        assert_eq!(to_json(&Ordered(ordered)), ordered);
        let numeric_after_strings = r#"{"z":0,"10":0,"2":0,"a":0}"#;
        assert_eq!(to_json(&Ordered(numeric_after_strings)), r#"{"2":0,"10":0,"z":0,"a":0}"#);
        let error = try_to_json(&std::collections::BTreeMap::from([("depth", [f64::NEG_INFINITY])])).unwrap_err();
        assert_eq!(
            error.to_string(),
            r#"loft: internal: build: manifest field "0" is -Infinity, which JSON cannot hold"#
        );

        let references = references();
        for dir in &references {
            let path = dir.join(MANIFEST);
            let source = fs::read_to_string(&path).unwrap();
            let (value, tail) = split_json(&source);
            let output = to_json(&Ordered(value));
            if output != source {
                let first = source
                    .bytes()
                    .zip(output.bytes())
                    .position(|(a, b)| a != b)
                    .unwrap_or(source.len().min(output.len()));
                let take =
                    |s: &str| String::from_utf8_lossy(&s.as_bytes()[first..(first + 64).min(s.len())]).into_owned();
                panic!("{} at byte {first}: TS {:?}, Rust {:?}", path.display(), take(&source), take(&output));
            }
            assert!(tail.is_empty(), "{}", path.display());
        }
        println!("{} TS manifests matched byte for byte", references.len());
    }

    #[test]
    fn chunk_names_match_ts_files() {
        // Catches hashing UTF-8 bytes, Unicode scalar values, or shifting the updated second hash.
        assert_eq!(chunk_name("// café 🦀\n"), "05785224c7b58236");
        let references = references();
        let mut count = 0;
        for dir in &references {
            for entry in fs::read_dir(dir.join("wgsl")).unwrap() {
                let path = entry.unwrap().path();
                let text = fs::read_to_string(&path).unwrap();
                assert_eq!(chunk_name(&text), path.file_stem().unwrap().to_str().unwrap(), "{}", path.display());
                count += 1;
            }
        }
        println!("{count} named chunks matched across {} TS artefacts", references.len());
    }

    #[test]
    fn bundle_replacement_checks_first() {
        // Catches deleting earlier build files before refusing a cpu table or a stray file, and stale chunk leakage.
        let dir = Path::new(ROOT).join(format!("out/sw/bundle-{}", std::process::id()));
        let manifest = r#"{"tables":[{"name":"pose","fill":{"kind":"gpu","stage":3}}]}"#;
        let chunks = [("draw".into(), "shader\n".into())];
        let blobs = [("rows".into(), vec![0, 1, 2])];
        write_bundle(&dir, manifest, &chunks, &blobs).unwrap();
        assert_eq!(fs::read_to_string(dir.join(MANIFEST)).unwrap(), manifest);
        assert_eq!(fs::read_to_string(dir.join("wgsl/draw.wgsl")).unwrap(), "shader\n");
        assert_eq!(fs::read(dir.join("bin/rows.bin")).unwrap(), [0, 1, 2]);
        let cpu = r#"{"tables":[{"name":"pose","fill":{"kind":"cpu","why":"uses JS"}}]}"#;
        let err = write_bundle(&dir, cpu, &[], &[]).unwrap_err();
        assert!(matches!(err, Error::Scene(head) if head.ends_with(r#""pose" (uses JS)"#)));
        assert!(dir.join("wgsl/draw.wgsl").is_file());
        fs::write(dir.join("keep"), "stray").unwrap();
        let err = write_bundle(&dir, manifest, &[], &[]).unwrap_err();
        assert!(matches!(err, Error::Internal(head) if head.contains("holds keep")));
        assert!(dir.join("wgsl/draw.wgsl").is_file());
        fs::remove_file(dir.join("keep")).unwrap();
        write_bundle(&dir, manifest, &[], &[]).unwrap();
        assert_eq!(fs::read_dir(dir.join("wgsl")).unwrap().count(), 0);
        assert_eq!(fs::read_dir(dir.join("bin")).unwrap().count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }
}
