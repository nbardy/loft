/// Stable symbol word also used by the host when looking up an authored tag.
pub fn event_tag(name: &str) -> u32 {
    u32::from_le_bytes(
        blake3::hash(name.as_bytes()).as_bytes()[..4]
            .try_into()
            .unwrap(),
    )
}

/// Bind the returned outbox as read/write storage in the pass declaration.
/// `tick` is the tick expression in the writer's scope.
pub fn event_wgsl(group: u8, slot: u32, tick: &str) -> String {
    format!(
        r#"
struct OutboxEvent {{ tag: u32, instance_id: u32, site_id: u32, tick: u32, payload: vec4<f32> }}
struct OutboxHeader {{ write_count: atomic<u32>, capacity: u32, dropped: atomic<u32>, frame_idx: u32 }}
struct Outbox {{ header: OutboxHeader, events: array<OutboxEvent> }}
@group({group}) @binding({slot}) var<storage, read_write> loft_outbox: Outbox;
fn emit_event(tag: u32, instance_id: u32, site_id: u32, payload: vec4<f32>) {{
    let slot = atomicAdd(&loft_outbox.header.write_count, 1u);
    if slot < loft_outbox.header.capacity {{
        loft_outbox.events[slot] = OutboxEvent(tag, instance_id, site_id, {tick}, payload);
    }} else {{
        atomicAdd(&loft_outbox.header.dropped, 1u);
    }}
}}
"#
    )
}
