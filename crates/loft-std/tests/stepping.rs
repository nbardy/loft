use loft_std::stepping::chunks;

#[test]
fn checkpoint_windows() {
    let windows: Vec<_> = chunks(58, 130, 64, 60)
        .map(|a| (a.from_tick, a.to_tick))
        .collect();
    assert_eq!(windows, [(58, 60), (60, 120), (120, 130)]);
    assert_eq!(chunks(8, 8, 64, 0).count(), 1);
}
