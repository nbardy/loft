use loft_abi::Tier0Opcode::*;

pub const CONSTANTS: [[f32; 4]; 2] = [[0.5, 1., 2., 4.], [2.; 4]];
pub const POINTS: [[f32; 4]; 2] = [[0.; 4], [1., 0., 0., 0.]];

pub fn cases() -> [(&'static str, Vec<u32>, [[f32; 4]; 2]); 7] {
    let ret = |a| Ret.pack(0, a, 0, 0).unwrap();
    let load = |dst, slot| LoadConst.pack(dst, 0, 0, slot).unwrap();
    let nop = Nop.pack(0, 0, 0, 0).unwrap();
    let fma = |c| Fma.pack(0, 2, 2, c).unwrap();
    let add = |b| Add.pack(0, 0, b, 0).unwrap();
    let eight = vec![
        load(3, 0),
        load(2, 1),
        load(1, 0),
        fma(3),
        add(1),
        nop,
        nop,
        ret(0),
    ];
    let nine = vec![
        load(3, 0),
        load(2, 1),
        load(1, 0),
        fma(3),
        add(1),
        Neg.pack(0, 0, 0, 0).unwrap(),
        nop,
        nop,
        ret(0),
    ];
    [
        ("four-registers-eight-words", eight, [[5., 6., 8., 12.]; 2]),
        (
            "four-registers-nine-words",
            nine,
            [[-5., -6., -8., -12.]; 2],
        ),
        (
            "five-registers",
            vec![
                load(4, 0),
                load(3, 0),
                load(2, 1),
                load(1, 0),
                fma(4),
                add(3),
                add(1),
                ret(0),
            ],
            [[5.5, 7., 10., 16.]; 2],
        ),
        ("register-31", vec![load(31, 0), ret(31)], [CONSTANTS[0]; 2]),
        (
            "loop",
            vec![
                load(0, 1),
                LoopBegin.pack(0, 0, 0, 3).unwrap(),
                Add.pack(1, 1, 0, 0).unwrap(),
                LoopEnd.pack(0, 0, 0, 1).unwrap(),
                ret(1),
            ],
            [[4.; 4]; 2],
        ),
        (
            "swizzle-immediate",
            vec![load(0, 0), Swizzle.pack(1, 0, 0, 0x1b).unwrap(), ret(1)],
            [[4., 2., 1., 0.5]; 2],
        ),
        (
            "box-edit",
            vec![
                LoadRow.pack(0, 0, 0, 0).unwrap(),
                load(1, 0),
                SdBox.pack(2, 0, 1, 0).unwrap(),
                ret(2),
            ],
            [[-0.5; 4], [0.5; 4]],
        ),
    ]
}

pub fn save_words(name: &str, words: impl IntoIterator<Item = u32>) {
    if let Some(dir) = std::env::var_os("LOFT_TIER0_EVIDENCE") {
        let bytes: Vec<_> = words.into_iter().flat_map(u32::to_le_bytes).collect();
        std::fs::write(std::path::Path::new(&dir).join(name), bytes).unwrap();
    }
}
