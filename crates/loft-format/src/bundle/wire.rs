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
// Every lane has a fixed width and a zero initializer. Bytes above use a bulk read.
macro_rules! numeric_arrays {
    ($($ty:ty),+) => {$(
        impl<const N: usize> Wire for [$ty; N] {
            fn put(&self, out: &mut Vec<u8>) {
                for value in self {
                    value.put(out);
                }
            }

            fn get(input: &mut Reader<'_>) -> Result<Self> {
                let mut values = [<$ty>::default(); N];
                for value in &mut values {
                    *value = <$ty>::get(input)?;
                }
                Ok(values)
            }
        }
    )+};
}
numeric_arrays!(f32, f64, u32);

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

impl<T: Wire> Wire for Box<[T]> {
    fn put(&self, out: &mut Vec<u8>) {
        put_slice(self, out);
    }
    fn get(input: &mut Reader<'_>) -> Result<Self> {
        Ok(Vec::get(input)?.into_boxed_slice())
    }
}

fn put_slice<T: Wire>(values: &[T], out: &mut Vec<u8>) {
    (values.len() as u64).put(out);
    for value in values {
        value.put(out);
    }
}

impl<T: Wire> Wire for Vec<T> {
    fn put(&self, out: &mut Vec<u8>) {
        put_slice(self, out);
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

macro_rules! sum {
    (pub enum $(#[$attr:meta])* $ty:ident {
        $($tag:literal => $variant:ident $({ $($field:ident: $field_ty:ty),+ $(,)? })?),+ $(,)?
    }) => {
        $(#[$attr])*
        pub enum $ty { $($variant $({ $($field: $field_ty),+ })?),+ }
        $crate::bundle::wire::sum! {
            $ty { $($tag => $variant $({ $($field: $field_ty),+ })?),+ }
        }
    };
    ($(#[$attr:meta])* $ty:ident {
        $($tag:literal => $variant:ident
            $({ $($field:ident: $field_ty:ty),+ $(,)? })?
            $(($item:ident: $item_ty:ty))?),+ $(,)?
    }) => {
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
                    $($tag => Self::$variant $({ $($field: <$field_ty>::get(input)?),+ })?
                        $( ( <$item_ty>::get(input)? ) )?,)+
                    _ => return Err(BundleError::Encoding),
                })
            }
        }
    };
}

pub(crate) use {fields, sum};
