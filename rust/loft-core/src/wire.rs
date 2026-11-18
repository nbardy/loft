//! The bytes between Bun and the core: little-endian, read in order. JS writes with src/core/wire.ts.

#[derive(Debug)]
pub struct WireError(pub String);

pub struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub fn new(b: &'a [u8]) -> Self { Reader { b, at: 0 } }
    fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        if self.at + n > self.b.len() {
            return Err(WireError(format!("read {} bytes at {} of {}", n, self.at, self.b.len())));
        }
        let s = &self.b[self.at..self.at + n];
        self.at += n;
        Ok(s)
    }
    pub fn u32(&mut self) -> Result<u32, WireError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn i32(&mut self) -> Result<i32, WireError> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn f64(&mut self) -> Result<f64, WireError> {
        // JS pads to 8 before every f64 run, so this is always aligned in the source buffer; read bytewise anyway.
        Ok(f64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn usize(&mut self) -> Result<usize, WireError> { Ok(self.u32()? as usize) }
    pub fn f64s(&mut self) -> Result<Vec<f64>, WireError> {
        let n = self.usize()?;
        (0..n).map(|_| self.f64()).collect()
    }
    pub fn u32s(&mut self) -> Result<Vec<u32>, WireError> {
        let n = self.usize()?;
        (0..n).map(|_| self.u32()).collect()
    }
    pub fn str(&mut self) -> Result<String, WireError> {
        let n = self.usize()?;
        let s = self.take(n)?;
        String::from_utf8(s.to_vec()).map_err(|e| WireError(format!("utf8: {e}")))
    }
    pub fn done(&self) -> Result<(), WireError> {
        if self.at == self.b.len() { Ok(()) } else { Err(WireError(format!("{} bytes left over", self.b.len() - self.at))) }
    }
}

#[derive(Default)]
pub struct Writer {
    pub b: Vec<u8>,
}

impl Writer {
    pub fn u8(&mut self, v: u8) { self.b.push(v); }
    pub fn u32(&mut self, v: u32) { self.b.extend_from_slice(&v.to_le_bytes()); }
    pub fn f64(&mut self, v: f64) { self.b.extend_from_slice(&v.to_le_bytes()); }
    pub fn f32(&mut self, v: f32) { self.b.extend_from_slice(&v.to_le_bytes()); }
    pub fn f64s(&mut self, v: &[f64]) { self.u32(v.len() as u32); for x in v { self.f64(*x); } }
    pub fn u32s(&mut self, v: &[u32]) { self.u32(v.len() as u32); for x in v { self.u32(*x); } }
    pub fn str(&mut self, s: &str) { self.u32(s.len() as u32); self.b.extend_from_slice(s.as_bytes()); }
}
