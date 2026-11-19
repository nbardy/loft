//! Decode the fixed event outbox layout.
use loft_ir::OutboxEvent;
pub const HEADER: usize = 16;
pub const RECORD: usize = 32;
pub type Events = (Vec<OutboxEvent>, u32);

#[derive(Debug, PartialEq, Eq)]
pub struct InvalidOutbox;

pub fn capacity(size: u64) -> Result<u32, InvalidOutbox> {
    (size >= (HEADER + RECORD) as u64 && (size - HEADER as u64).is_multiple_of(RECORD as u64)
        || size == 16_384)
        .then(|| (size - HEADER as u64) / RECORD as u64)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or(InvalidOutbox)
}

pub fn decode(bytes: &[u8], frame: u32) -> Result<Events, InvalidOutbox> {
    let n = capacity(bytes.len() as u64)?;
    let word = |i: usize| u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap());
    if word(1) != n || word(3) != frame || word(2) != word(0).saturating_sub(n) {
        return Err(InvalidOutbox);
    }
    let events = (0..word(0).min(n) as usize)
        .map(|i| {
            let j = 4 + i * 8;
            OutboxEvent {
                tag: word(j),
                instance_id: word(j + 1),
                site_id: word(j + 2),
                tick: word(j + 3),
                payload: std::array::from_fn(|k| f32::from_bits(word(j + 4 + k))),
            }
        })
        .collect();
    Ok((events, word(2)))
}
