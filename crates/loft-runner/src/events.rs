//! Sparse event readback. The queue owns copies and resets in submission order.
use super::*;
use std::{collections::VecDeque, sync::Mutex};
use wgpu::util::DeviceExt;

pub use loft_ir::OutboxEvent;
const HEADER: usize = 16;
const RECORD: usize = 32;

type Events = (Vec<OutboxEvent>, u32);
type Completion = Arc<Mutex<Option<Result<Events>>>>;

struct Slot {
    buffer: wgpu::Buffer,
    pending: Option<(u32, Completion)>,
}

pub(super) struct Outbox {
    slots: [Slot; 3],
    ready: VecDeque<(u32, Result<Events>)>,
    frame: u32,
}
impl Outbox {
    pub(super) fn new(device: &wgpu::Device, size: u64) -> Result<Self> {
        capacity(size)?;
        Ok(Self {
            slots: std::array::from_fn(|_| Slot {
                buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("event staging"),
                    size,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                pending: None,
            }),
            ready: VecDeque::new(),
            frame: 0,
        })
    }

    pub(super) fn prepare(&mut self) -> Result<()> {
        let slot = &mut self.slots[self.frame as usize % 3];
        if let Some((frame, done)) = &slot.pending {
            let events = done.lock().unwrap().take().ok_or(RunnerError::Capacity)?;
            self.ready.push_back((*frame, events));
            slot.pending = None;
        }
        self.frame.checked_add(1).ok_or(RunnerError::Capacity)?;
        Ok(())
    }

    pub(super) fn begin(
        &self,
        device: &wgpu::Device,
        enc: &mut wgpu::CommandEncoder,
        outbox: &wgpu::Buffer,
    ) -> Result<()> {
        let header = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("outbox capacity and frame"),
            usage: wgpu::BufferUsages::COPY_SRC,
            contents: [
                capacity(outbox.size())?.to_le_bytes(),
                self.frame.to_le_bytes(),
            ]
            .as_flattened(),
        });
        enc.copy_buffer_to_buffer(&header, 0, outbox, 4, Some(4));
        enc.copy_buffer_to_buffer(&header, 4, outbox, 12, Some(4));
        Ok(())
    }

    pub(super) fn end(&mut self, enc: &mut wgpu::CommandEncoder, outbox: &wgpu::Buffer) {
        let frame = self.frame;
        let slot = &mut self.slots[frame as usize % 3];
        let copy = slot.buffer.clone();
        enc.copy_buffer_to_buffer(outbox, 0, &copy, 0, Some(copy.size()));
        // Preserve capacity. Each frame gets its own overflow count.
        enc.clear_buffer(outbox, 0, Some(4));
        enc.clear_buffer(outbox, 8, Some(4));
        let done: Completion = Arc::new(Mutex::new(None));
        slot.pending = Some((frame, done.clone()));
        enc.map_buffer_on_submit(&slot.buffer, wgpu::MapMode::Read, .., move |status| {
            let result = status
                .as_ref()
                .map_err(|_| RunnerError::Unsupported("outbox mapping failed"))
                .and_then(|()| {
                    let bytes = copy
                        .get_mapped_range(..)
                        .map_err(|_| RunnerError::Unsupported("outbox mapping failed"))?;
                    decode(&bytes, frame)
                });
            if status.is_ok() {
                copy.unmap();
            }
            *done.lock().unwrap() = Some(result);
        });
        self.frame += 1;
    }

    fn drain(&mut self) -> Result<Events> {
        let Some(eligible) = self.frame.checked_sub(3) else {
            return Ok((vec![], 0));
        };
        if self
            .ready
            .front()
            .is_some_and(|(frame, _)| *frame <= eligible)
        {
            return self.ready.pop_front().unwrap().1;
        }
        let pending = self
            .slots
            .iter_mut()
            .filter_map(|s| {
                let (frame, _) = s.pending.as_ref()?;
                (*frame <= eligible).then_some((*frame, s))
            })
            .min_by_key(|(f, _)| *f);
        if let Some((_, slot)) = pending {
            let (_, done) = slot.pending.as_ref().unwrap();
            let completed = done.lock().unwrap().take();
            if let Some(events) = completed {
                slot.pending = None;
                return events;
            }
        }
        Ok((vec![], 0))
    }
}

impl Runner {
    /// Take the oldest completed outbox eligible for the two-frame readback delay.
    pub fn drain_events(&mut self, id: InstanceId) -> Result<(Vec<OutboxEvent>, u32)> {
        let _ = self.device.poll(wgpu::PollType::Poll);
        self.instance(id)?
            .outbox
            .as_mut()
            .map_or_else(|| Ok((vec![], 0)), Outbox::drain)
    }
}

pub(super) fn capacity(size: u64) -> Result<u32> {
    (size >= (HEADER + RECORD) as u64 && (size - HEADER as u64).is_multiple_of(RECORD as u64)
        || size == 16_384)
        .then(|| (size - HEADER as u64) / RECORD as u64)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or(RunnerError::AbiMismatch)
}

fn decode(bytes: &[u8], frame: u32) -> Result<Events> {
    let n = capacity(bytes.len() as u64)?;
    let word = |i: usize| u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap());
    if word(1) != n || word(3) != frame || word(2) != word(0).saturating_sub(n) {
        return Err(RunnerError::AbiMismatch);
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

#[cfg(test)]
#[test]
fn outbox_record_prefix() {
    let mut bytes = [0u8; HEADER + RECORD];
    let words = [2u32, 1, 1, 7, 10, 11, 12, 13, 0, 0, 0, 1.0f32.to_bits()];
    bytes.copy_from_slice(words.map(u32::to_le_bytes).as_flattened());
    let (events, dropped) = decode(&bytes, 7).unwrap();
    assert_eq!(dropped, 1);
    assert_eq!(
        (
            events[0].tag,
            events[0].instance_id,
            events[0].site_id,
            events[0].tick
        ),
        (10, 11, 12, 13)
    );
    assert_eq!(events[0].payload, [0., 0., 0., 1.]);
    assert!(decode(&bytes, 6).is_err());
    assert_eq!(capacity(16_384), Ok(511));
}
