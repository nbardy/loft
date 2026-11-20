//! Extension declarations stored in the bundle's DATA section.
//! A reserved table with a byte stride is rejected by readers that do not know the extension.
use crate::{ElemFormat, PassId, TableDecl, TableId, TableLifetime};

pub const DESCRIPTOR: TableId = TableId(u32::MAX);
const VERSION: u32 = 2;

#[derive(Default)]
pub struct Execution {
    pub initializers: Vec<Initializer>,
    pub additive: Vec<PassId>,
    pub ranges: Vec<BufferView>,
}
pub struct BufferView {
    pub pass: PassId,
    pub group: u8,
    pub slot: u32,
    pub offset: u64,
    pub size: u64,
}
pub struct Initializer {
    pub name: String,
    pub passes: Vec<PassId>,
    pub clock: Clock,
    pub image: Option<Image>,
}
pub enum Clock {
    Load,
    Dirty(Vec<usize>),
}
pub struct Image {
    pub table: TableId,
    pub dimension: Dimension,
    pub extent: [u32; 3],
    pub levels: u32,
    pub format: ImageStorage,
}
#[derive(Clone, Copy)]
pub enum Dimension {
    Plane,
    Volume,
    Cube,
}

#[derive(Clone, Copy)]
pub enum ImageStorage {
    LegacyReflect,
    R8Unorm,
    R16Float,
    R32Float,
    Rgba8Unorm,
    Rgba16Float,
}

impl ImageStorage {
    fn encode(self) -> u32 {
        match self {
            Self::LegacyReflect => 0, Self::R8Unorm => 1, Self::R16Float => 2,
            Self::R32Float => 3, Self::Rgba8Unorm => 4, Self::Rgba16Float => 5,
        }
    }

    fn decode(word: u32) -> Option<Self> {
        Some(match word {
            0 => Self::LegacyReflect, 1 => Self::R8Unorm, 2 => Self::R16Float,
            3 => Self::R32Float, 4 => Self::Rgba8Unorm, 5 => Self::Rgba16Float,
            _ => return None,
        })
    }
}

impl Execution {
    #[allow(dead_code)]
    pub fn table(&self) -> Option<TableDecl> {
        if self.initializers.is_empty() && self.additive.is_empty() && self.ranges.is_empty() { return None; }
        let mut bytes = VERSION.to_le_bytes().to_vec();
        let put = |bytes: &mut Vec<u8>, word: u32| bytes.extend(word.to_le_bytes());
        put(&mut bytes, self.initializers.len() as u32);
        for init in &self.initializers {
            put(&mut bytes, init.name.len() as u32);
            bytes.extend(init.name.as_bytes());
            put(&mut bytes, init.passes.len() as u32);
            for id in &init.passes { put(&mut bytes, id.0); }
            match &init.clock {
                Clock::Load => put(&mut bytes, 0),
                Clock::Dirty(words) => {
                    put(&mut bytes, 1);
                    put(&mut bytes, words.len() as u32);
                    for &word in words { put(&mut bytes, word as u32); }
                }
            }
            match &init.image {
                None => put(&mut bytes, 0),
                Some(image) => {
                    put(&mut bytes, match image.dimension { Dimension::Plane => 1, Dimension::Volume => 2, Dimension::Cube => 3 });
                    put(&mut bytes, image.table.0);
                    for n in image.extent { put(&mut bytes, n); }
                    put(&mut bytes, image.levels);
                    put(&mut bytes, image.format.encode());
                }
            }
        }
        put(&mut bytes, self.additive.len() as u32);
        for id in &self.additive { put(&mut bytes, id.0); }
        put(&mut bytes, self.ranges.len() as u32);
        for range in &self.ranges {
            put(&mut bytes, range.pass.0);
            put(&mut bytes, u32::from(range.group));
            put(&mut bytes, range.slot);
            bytes.extend(range.offset.to_le_bytes());
            bytes.extend(range.size.to_le_bytes());
        }
        Some(TableDecl { id: DESCRIPTOR, format: ElemFormat::U8, lifetime: TableLifetime::Static,
            stride_bytes: 1, capacity: bytes.len() as u32, initial: bytes.into() })
    }

    #[allow(dead_code)]
    pub fn read(table: &TableDecl) -> Option<Self> {
        struct Reader<'a>(&'a [u8]);
        impl<'a> Reader<'a> {
            fn take(&mut self, count: usize) -> Option<&'a [u8]> {
                let (bytes, rest) = self.0.split_at_checked(count)?;
                self.0 = rest;
                Some(bytes)
            }
            fn word(&mut self) -> Option<u32> { Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?)) }
            fn long(&mut self) -> Option<u64> { Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?)) }
            fn list<T>(&mut self, mut item: impl FnMut(&mut Self) -> Option<T>) -> Option<Vec<T>> {
                let count = self.word()? as usize;
                if count > self.0.len() / 4 { return None; }
                (0..count).map(|_| item(self)).collect()
            }
        }
        if table.stride_bytes != 1 || table.capacity as usize != table.initial.len()
            || table.format != ElemFormat::U8 || table.lifetime != TableLifetime::Static { return None; }
        let mut rd = Reader(&table.initial);
        let legacy = match rd.word()? {
            1 => true,
            VERSION => false,
            _ => return None,
        };
        let initializers = rd.list(|r| {
            let len = r.word()? as usize;
            let name = std::str::from_utf8(r.take(len)?).ok()?.to_owned();
            let passes = r.list(|r| Some(PassId(r.word()?)))?;
            let clock = match r.word()? {
                0 => Clock::Load,
                1 => Clock::Dirty(r.list(|r| Some(r.word()? as usize))?),
                _ => return None,
            };
            let dim = match r.word()? {
                0 => None,
                1 => Some(Dimension::Plane),
                2 => Some(Dimension::Volume),
                3 => Some(Dimension::Cube),
                _ => return None,
            };
            let image = if let Some(dimension) = dim {
                Some(Image { table: TableId(r.word()?), dimension, extent: [r.word()?, r.word()?, r.word()?], levels: r.word()?,
                    format: if legacy { ImageStorage::LegacyReflect } else { ImageStorage::decode(r.word()?)? } })
            } else { None };
            Some(Initializer { name, passes, clock, image })
        })?;
        let additive = rd.list(|r| Some(PassId(r.word()?)))?;
        let ranges = if legacy { vec![] } else { rd.list(|r| Some(BufferView {
            pass: PassId(r.word()?), group: r.word()?.try_into().ok()?, slot: r.word()?,
            offset: r.long()?, size: r.long()?,
        }))? };
        rd.0.is_empty().then_some(Self { initializers, additive, ranges })
    }
}
