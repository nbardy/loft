/// Interpret the platform's shared visibility record in its original 16-bit lanes.
pub(super) fn visibility_ids(id: u32, payload: u32) -> (u32, u32) {
    let (kind, _, row, _) = loft_abi::VisRec([
        id as u16,
        (id >> 16) as u16,
        payload as u16,
        (payload >> 16) as u16,
    ])
    .unpack();
    (kind, row)
}
