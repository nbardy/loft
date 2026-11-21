use loft_std::events::{HEADER, RECORD, capacity, decode};

#[test]
fn outbox_record_prefix() {
    let mut bytes = [0u8; HEADER + RECORD];
    let words = [2u32, 1, 1, 7, 10, 11, 12, 13, 0, 0, 0, 1.0f32.to_bits()];
    bytes.copy_from_slice(words.map(u32::to_le_bytes).as_flattened());
    let (events, dropped) = decode(&bytes, 7).unwrap();
    assert_eq!(dropped, 1);
    assert_eq!(
        (events[0].tag, events[0].instance_id, events[0].site_id, events[0].tick),
        (10, 11, 12, 13)
    );
    assert_eq!(events[0].payload, [0., 0., 0., 1.]);
    assert!(decode(&bytes, 6).is_err());
    assert_eq!(capacity(16_384), Ok(511));
}
