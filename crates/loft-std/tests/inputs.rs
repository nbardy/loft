use loft_ir::{InputId, InputKind, InputSlot, InputValue};
use loft_std::inputs::Inputs;

#[test]
fn idle_frames_skip_uploads_and_events_clear_once() {
    let slots = [InputKind::Level, InputKind::Event, InputKind::Packet].map(|kind| InputSlot {
        kind,
        offset_words: match kind {
            InputKind::Level => 0,
            InputKind::Event => 1,
            InputKind::Packet => 2,
        },
        words: 1,
    });
    let mut inputs = Inputs::new(&slots, 16).unwrap();
    let word = |bytes: &[u8], i| f32::from_le_bytes(bytes.as_chunks::<4>().0[i]);

    assert_eq!(inputs.advance(0).unwrap().unwrap(), [0; 16]);
    assert!(inputs.advance(1).unwrap().is_none());
    inputs.push(InputId(0), 4, InputValue::Level(&[3.0])).unwrap();
    inputs.push(InputId(1), 4, InputValue::Event { seq: 1, payload: &[5.0] }).unwrap();
    inputs.push(InputId(2), 4, InputValue::PosePacket { tick: 4, data: &[7.0] }).unwrap();
    assert!(inputs.advance(2).unwrap().is_none());
    assert!(inputs.advance(3).unwrap().is_none());
    let data = inputs.advance(4).unwrap().unwrap();
    assert_eq!([word(data, 0), word(data, 1), word(data, 2)], [3.0, 5.0, 7.0]);
    let data = inputs.advance(4).unwrap().unwrap();
    assert_eq!([word(data, 0), word(data, 1), word(data, 2)], [3.0, 0.0, 7.0]);

    inputs.push(InputId(1), 5, InputValue::Event { seq: 1, payload: &[9.0] }).unwrap();
    assert!(inputs.advance(5).unwrap().is_none());
    inputs.push(InputId(1), 6, InputValue::Event { seq: 2, payload: &[9.0] }).unwrap();
    assert!(inputs.advance(7).unwrap().is_none());
}
