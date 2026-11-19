use super::*;

pub(super) struct Inputs {
    slots: Vec<InputSlot>,
    pending: BTreeMap<(u64, u32), Vec<u8>>,
    event_seq: BTreeMap<u32, u64>,
    bytes: Vec<u8>,
    tick: Option<u64>,
}
impl Inputs {
    pub fn new(slots: &[InputSlot], max_bytes: u64) -> Result<Self> {
        let mut ranges = Vec::new();
        for slot in slots {
            let end = slot.offset_words.checked_add(slot.words)
                .filter(|&end| u64::from(end) <= max_bytes / 4 && slot.words > 0)
                .ok_or(RunnerError::Capacity)?;
            ranges.push((slot.offset_words, end));
        }
        ranges.sort_unstable();
        if ranges.windows(2).any(|r| r[0].1 > r[1].0) { return Err(RunnerError::AbiMismatch); }
        let size = ranges.last().map_or(4, |r| r.1).max(4);
        Ok(Self { slots: slots.into(), pending: BTreeMap::new(), event_seq: BTreeMap::new(),
            bytes: vec![0; size as usize * 4], tick: None })
    }

    pub fn size(&self) -> u64 { self.bytes.len() as u64 }

    pub fn push(&mut self, id: InputId, tick: u64, value: InputValue<'_>) -> Result<()> {
        let slot = self.slots.get(id.0 as usize).ok_or(RunnerError::UnknownResource)?;
        let (kind, seq, values) = match value {
            InputValue::Level(data) => (InputKind::Level, None, data),
            InputValue::Event { seq, payload } => (InputKind::Event, Some(seq), payload),
            InputValue::PosePacket { tick: at, data } if at == tick => (InputKind::Packet, None, data),
            InputValue::PosePacket { .. } => return Err(RunnerError::AbiMismatch),
        };
        if kind != slot.kind || values.len() != slot.words as usize || values.iter().any(|v| !v.is_finite())
            || self.tick.is_some_and(|last| tick <= last)
        {
            return Err(RunnerError::AbiMismatch);
        }
        if let Some(seq) = seq {
            if self.event_seq.get(&id.0).is_some_and(|&last| seq <= last) { return Ok(()); }
            self.event_seq.insert(id.0, seq);
        }
        self.pending.insert((tick, id.0), values.iter().flat_map(|v| v.to_le_bytes()).collect());
        Ok(())
    }

    pub fn advance(&mut self, tick: u64) -> Result<Option<&[u8]>> {
        if self.slots.is_empty() { return Ok(None); }
        if self.tick.is_some_and(|last| tick < last) { return Err(RunnerError::AbiMismatch); }
        for slot in self.slots.iter().filter(|s| s.kind == InputKind::Event) {
            self.bytes[range(slot)].fill(0);
        }
        while let Some((&(at, id), _)) = self.pending.first_key_value() {
            if at > tick { break; }
            let (_, bytes) = self.pending.pop_first().unwrap();
            let slot = &self.slots[id as usize];
            if at == tick || slot.kind != InputKind::Event {
                self.bytes[range(slot)].copy_from_slice(&bytes);
            }
        }
        self.tick = Some(tick);
        Ok(Some(&self.bytes))
    }
}

fn range(slot: &InputSlot) -> std::ops::Range<usize> {
    (slot.offset_words as usize * 4)..((slot.offset_words + slot.words) as usize * 4)
}
