//! Fixed-width, little-endian bincode representation, using the frozen declaration order.
use super::BundleError;

pub(crate) type Result<T> = std::result::Result<T, BundleError>;

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    nesting: u8,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, nesting: 0 }
    }

    pub fn nested<T>(&mut self, read: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        if self.nesting == 32 {
            return Err(BundleError::Encoding);
        }
        self.nesting += 1;
        let result = read(self);
        self.nesting -= 1;
        result
    }

    pub fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let (value, rest) = self
            .bytes
            .split_at_checked(len)
            .ok_or(BundleError::Truncated)?;
        self.bytes = rest;
        Ok(value)
    }

    pub fn finish(self) -> Result<()> {
        if self.bytes.is_empty() {
            Ok(())
        } else {
            Err(BundleError::Encoding)
        }
    }
}

pub(crate) trait Wire: Sized {
    fn put(&self, out: &mut Vec<u8>);
    fn get(input: &mut Reader<'_>) -> Result<Self>;
}

macro_rules! number {
    ($($ty:ty),+) => {$(
        impl Wire for $ty {
            fn put(&self, out: &mut Vec<u8>) {
                out.extend_from_slice(&self.to_le_bytes());
            }
            fn get(input: &mut Reader<'_>) -> $crate::bundle::wire::Result<Self> {
                Ok(Self::from_le_bytes(input.take(size_of::<Self>())?.try_into().unwrap()))
            }
        }
    )+};
}
number!(u8, u16, u32, u64, f32, f64);

impl<const N: usize> Wire for [u8; N] {
    fn put(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(self);
    }
    fn get(input: &mut Reader<'_>) -> Result<Self> {
        Ok(input.take(N)?.try_into().unwrap())
    }
}
impl<const N: usize> Wire for [f32; N] {
    fn put(&self, out: &mut Vec<u8>) {
        for value in self {
            value.put(out);
        }
    }
    fn get(input: &mut Reader<'_>) -> Result<Self> {
        let mut values = [0.; N];
        for value in &mut values {
            *value = f32::get(input)?;
        }
        Ok(values)
    }
}
impl Wire for [f64; 4] {
    fn put(&self, out: &mut Vec<u8>) {
        for value in self { value.put(out); }
    }
    fn get(input: &mut Reader<'_>) -> Result<Self> {
        Ok([f64::get(input)?, f64::get(input)?, f64::get(input)?, f64::get(input)?])
    }
}
impl Wire for [u32; 3] {
    fn put(&self, out: &mut Vec<u8>) {
        for value in self {
            value.put(out);
        }
    }
    fn get(input: &mut Reader<'_>) -> Result<Self> {
        Ok([u32::get(input)?, u32::get(input)?, u32::get(input)?])
    }
}

impl Wire for bool {
    fn put(&self, out: &mut Vec<u8>) {
        u8::from(*self).put(out);
    }
    fn get(input: &mut Reader<'_>) -> Result<Self> {
        match u8::get(input)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(BundleError::Encoding),
        }
    }
}

impl<A: Wire, B: Wire> Wire for (A, B) {
    fn put(&self, out: &mut Vec<u8>) {
        self.0.put(out);
        self.1.put(out);
    }
    fn get(input: &mut Reader<'_>) -> Result<Self> {
        Ok((A::get(input)?, B::get(input)?))
    }
}

impl<T: Wire> Wire for Option<T> {
    fn put(&self, out: &mut Vec<u8>) {
        self.is_some().put(out);
        if let Some(value) = self {
            value.put(out);
        }
    }
    fn get(input: &mut Reader<'_>) -> Result<Self> {
        bool::get(input)?.then(|| T::get(input)).transpose()
    }
}

pub(crate) fn length(input: &mut Reader<'_>) -> Result<usize> {
    usize::try_from(u64::get(input)?)
        .ok()
        .filter(|&n| n <= input.bytes.len())
        .ok_or(BundleError::Truncated)
}

impl<T: Wire> Wire for Vec<T> {
    fn put(&self, out: &mut Vec<u8>) {
        (self.len() as u64).put(out);
        for value in self {
            value.put(out);
        }
    }
    fn get(input: &mut Reader<'_>) -> Result<Self> {
        // Every element on this wire takes at least one byte.
        (0..length(input)?).map(|_| T::get(input)).collect()
    }
}

impl Wire for String {
    fn put(&self, out: &mut Vec<u8>) {
        put_bytes(self.as_bytes(), out);
    }
    fn get(input: &mut Reader<'_>) -> Result<Self> {
        let bytes = get_bytes(input)?;
        Ok(std::str::from_utf8(bytes)
            .map_err(|_| BundleError::Encoding)?
            .into())
    }
}

pub(crate) fn put_bytes(bytes: &[u8], out: &mut Vec<u8>) {
    (bytes.len() as u64).put(out);
    out.extend_from_slice(bytes);
}

pub(crate) fn get_bytes<'a>(input: &mut Reader<'a>) -> Result<&'a [u8]> {
    let n = length(input)?;
    input.take(n)
}

macro_rules! fields {
    ($ty:ident { $($field:ident),+ $(,)? }) => {
        impl Wire for $ty {
            fn put(&self, out: &mut Vec<u8>) {
                let Self { $($field),+ } = self;
                $($field.put(out);)+
            }
            fn get(input: &mut Reader<'_>) -> $crate::bundle::wire::Result<Self> {
                Ok(Self { $($field: Wire::get(input)?),+ })
            }
        }
    };
}

macro_rules! variants {
    ($ty:ident { $($tag:literal => $variant:ident $({ $($field:ident),+ })? $(($item:ident))?),+ $(,)? }) => {
        impl Wire for $ty {
            fn put(&self, out: &mut Vec<u8>) {
                match self {
                    $(Self::$variant $({ $($field),+ })? $(($item))? => {
                        ($tag as u32).put(out);
                        $($($field.put(out);)+)?
                        $($item.put(out);)?
                    }),+
                }
            }
            fn get(input: &mut Reader<'_>) -> $crate::bundle::wire::Result<Self> {
                Ok(match u32::get(input)? {
                    $($tag => Self::$variant $({ $($field: Wire::get(input)?),+ })?
                       $(($crate::bundle::wire::get_named(input, stringify!($item))?))?,)+
                    _ => return Err(BundleError::Encoding),
                })
            }
        }
    };
}

pub(crate) fn get_named<T: Wire>(input: &mut Reader<'_>, _: &str) -> Result<T> {
    T::get(input)
}
pub(crate) use {fields, variants};
