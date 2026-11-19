use super::*;

fn word(op: Op, dst: u8, a: u8, b: u8, i: u16) -> u32 {
    op.pack(dst, a, b, i).unwrap()
}

fn assert_close(actual: V, expected: V, label: impl std::fmt::Debug) {
    for (a, e) in actual.into_iter().zip(expected) {
        assert!(
            (a - e).abs() <= 1e-5,
            "{label:?}: {actual:?} != {expected:?}"
        );
    }
}

#[test]
fn l23a_every_opcode_against_cpu_reference() {
    use Op::*;
    let zeros = [0.; 4];
    let fill = |v| [v; 4];
    let a = [0.5, 0.25, 0.75, 1.0];
    let b = [2., 3., 4., 5.];
    let c = [1., 0.5, 2., 1.];
    let constants = [
        a,
        b,
        c,
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [5., 6., 7., 1.],
        [-3., 0., 4., 8.],
        [0.; 4],
    ];
    let lut = [[1., 3., 0., 0.], [10.; 4], [20.; 4], [30.; 4]];
    let texels = [[1.; 4], [2.; 4], [3.; 4], [4.; 4]];
    let textures = [Texture {
        size: [2, 2],
        texels: &texels,
    }];
    let resources = Resources {
        constants: &constants,
        row: &[fill(60.)],
        inputs: &[fill(70.)],
        frame: &[fill(80.)],
        lut: &lut,
        textures: &textures,
    };
    let expected = [
        (Nop, 0, a),
        (LoadConst, 1, b),
        (LoadRow, 0, fill(60.)),
        (LoadInput, 0, fill(70.)),
        (LoadFrame, 0, fill(80.)),
        (Mov, 0, a),
        (Swizzle, 0x1b, [1., 0.75, 0.25, 0.5]),
        (Splat, 2, fill(0.75)),
        (Add, 0, [2.5, 3.25, 4.75, 6.]),
        (Sub, 0, [-1.5, -2.75, -3.25, -4.]),
        (Mul, 0, [1., 0.75, 3., 5.]),
        (Div, 0, [0.25, 1. / 12., 0.1875, 0.2]),
        (Fma, 2, [2., 1.25, 5., 6.]),
        (Neg, 0, [-0.5, -0.25, -0.75, -1.]),
        (Abs, 0, a),
        (Min, 0, a),
        (Max, 0, b),
        (Clamp, 2, c),
        (Mix, 2, [2., 1.625, 7.25, 5.]),
        (Select, 2, b),
        (Lt, 0, fill(1.)),
        (Le, 0, fill(1.)),
        (Eq, 0, zeros),
        (Step, 0, fill(1.)),
        (Smoothstep, 2, zeros),
        (Smootherstep, 2, zeros),
        (Floor, 0, [0., 0., 0., 1.]),
        (Fract, 0, [0.5, 0.25, 0.75, 0.]),
        (Sqrt, 0, [0.5f32.sqrt(), 0.5, 0.75f32.sqrt(), 1.]),
        (Sin, 0, a.map(f32::sin)),
        (Cos, 0, a.map(f32::cos)),
        (Atan2, 0, std::array::from_fn(|i| a[i].atan2(b[i]))),
        (Exp, 0, a.map(f32::exp)),
        (Log, 0, a.map(f32::ln)),
        (Pow, 0, [0.25, 0.015625, 0.31640625, 1.]),
        (Dot3, 0, fill(4.75)),
        (Cross, 0, [-1.25, -0.5, 1., 0.]),
        (
            LenNorm,
            0,
            [
                0.5 / 0.875f32.sqrt(),
                0.25 / 0.875f32.sqrt(),
                0.75 / 0.875f32.sqrt(),
                0.875f32.sqrt(),
            ],
        ),
        (XformPt, 3, [5.5, 6.25, 7.75, 1.]),
        (XformDir, 3, [0.5, 0.25, 0.75, 0.]),
        (MirrorX, 0, [3., 0., 4., -1.]),
        (RepeatCell, 0, a),
        (SdSphere, 0, fill(0.875f32.sqrt() - 2.)),
        (SdBox, 0, fill(-1.5)),
        (SdRoundBox, 2, fill(-1.5)),
        (
            SdCapsule,
            2,
            fill((0.875f32 - (4.75f32 * 4.75f32 / 29.)).sqrt() - 5.),
        ),
        (SminPoly, 1, fill(0.46875)),
        (SsubPoly, 1, fill(0.5)),
        (GradNoise3, 0, zeros),
        (SmoothVor3, 0, zeros),
        (Fbm2, 3, zeros),
        (SampleLutC2, 0, fill(20.)),
        (SampleTex2d, 0, fill(1.5)),
    ];
    let mut seen = [false; 56];
    let load = |r, i| word(LoadConst, r, 0, 0, i);
    let ret = |r| word(Ret, 0, r, 0, 0);
    for (op, imm, mut want) in expected {
        let lhs = match op {
            MirrorX => 7,
            GradNoise3 | SmoothVor3 | Fbm2 => 8,
            _ => 0,
        };
        let rhs = if op == SdCapsule { 8 } else { 1 };
        let third = if op == SdCapsule { 1 } else { 2 };
        if matches!(op, Smoothstep | Smootherstep) {
            // Independently expanded cubic/quintic coefficients.
            want = [1. / 3., 1. / 11., 5. / 13., 0.].map(|t: f32| {
                if op == Smoothstep {
                    3. * t.powi(2) - 2. * t.powi(3)
                } else {
                    6. * t.powi(5) - 15. * t.powi(4) + 10. * t.powi(3)
                }
            });
        }
        let words = [
            load(0, lhs),
            load(1, rhs),
            load(2, third),
            word(op, 0, 0, 1, imm),
            ret(0),
        ];
        assert_close(
            Tape::new(&words).unwrap().run(&resources).unwrap(),
            want,
            op,
        );
        seen[op as usize] = true;
    }
    for (count_slot, expected) in [(1, [4.5, 6.25, 8.75, 11.]), (8, a)] {
        let words = [
            load(0, 0),
            load(1, 1),
            load(2, count_slot),
            word(LoopBegin, 0, 2, 0, 5),
            word(Add, 0, 0, 1, 0),
            word(LoopEnd, 0, 0, 0, 3),
            ret(0),
        ];
        assert_close(
            Tape::new(&words).unwrap().run(&resources).unwrap(),
            expected,
            "loop + ret",
        );
    }
    for op in [LoopBegin, LoopEnd, Ret] {
        seen[op as usize] = true;
    }
    assert!(seen.into_iter().all(|op| op));
}

#[test]
fn l23a_linear_scan_reuses_dead_sources_and_refuses_spills() {
    use Op::*;
    let mut code: Vec<_> = (0..32)
        .map(|dst| Instruction {
            op: LoadConst,
            dst,
            args: [0; 3],
            imm: 0,
        })
        .collect();
    for next in 1..32 {
        let lhs = if next == 1 { 0 } else { next + 30 };
        code.push(Instruction {
            op: Add,
            dst: next + 31,
            args: [lhs, next, 0],
            imm: 0,
        });
    }
    code.push(Instruction {
        op: Ret,
        dst: 0,
        args: [62, 0, 0],
        imm: 0,
    });
    let words = allocate(&code).unwrap();
    let resources = Resources {
        constants: &[[1.; 4]],
        row: &[],
        inputs: &[],
        frame: &[],
        lut: &[],
        textures: &[],
    };
    assert_eq!(
        Tape::new(&words).unwrap().run(&resources).unwrap(),
        [32.; 4]
    );
    code.insert(
        32,
        Instruction {
            op: LoadConst,
            dst: 100,
            args: [0; 3],
            imm: 0,
        },
    );
    let last = code.len() - 1;
    code.insert(
        last,
        Instruction {
            op: Add,
            dst: 62,
            args: [62, 100, 0],
            imm: 0,
        },
    );
    assert_eq!(allocate(&code), Err(TapeError::Registers(32)));
}

#[test]
fn l23a_vm_validates_and_dispatches_all_frozen_ids() {
    let module = naga::front::wgsl::parse_str(VM_WGSL)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(VM_WGSL)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("{}", e.emit_to_string(VM_WGSL)));
    let vm = module
        .functions
        .iter()
        .find(|(_, f)| f.name.as_deref() == Some("loft_tier0"))
        .unwrap()
        .1;
    let mut cases = Vec::new();
    let mut pending = vec![&vm.body];
    while let Some(body) = pending.pop() {
        for stmt in body {
            match stmt {
                naga::Statement::Switch { cases: arms, .. } => {
                    cases.extend(arms.iter().filter_map(|a| match a.value {
                        naga::SwitchValue::U32(id) => Some(id),
                        naga::SwitchValue::I32(_) | naga::SwitchValue::Default => None,
                    }));
                }
                naga::Statement::Loop { body, .. } | naga::Statement::Block(body) => {
                    pending.push(body)
                }
                _ => {}
            }
        }
    }
    assert_eq!(cases, (0..56).collect::<Vec<_>>());
    assert_eq!(module.entry_points[0].workgroup_size, [32, 1, 1]);
}

#[test]
fn l23a_sphere_tape_matches_the_e_plus_twin() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../lib");
    let mut files = std::fs::read_dir(root)
        .unwrap()
        .map(|p| p.unwrap().path())
        .collect::<Vec<_>>();
    files.sort();
    let mut src = loft_ir::Sources::default();
    for file in files {
        src.add(
            file.display().to_string(),
            std::fs::read_to_string(&file).unwrap(),
        );
    }
    src.add(
        "edit.loft",
        "space World Body
param rough: f = 0.5 in 0.1..1
param sun_yaw: f = 0.25 in -1..1
kind K @Body (pose: xf@Body>World, radius: f > 0) {
  trace(p) = sphere(p - [0, 0, 0]@Body, radius)
  material(p, n) = ggx([0.5, 0.5, 0.5], rough, 0)
}
build { emit K(pose: place([0, 0, 0]@World), radius: 0.5) }",
    );
    let cache = tempfile::tempdir().unwrap();
    let (program, _, twin) = crate::compile_cached(&src, &[], cache.path())
        .unwrap_or_else(|d| panic!("{}", d.render(&src)));
    let meta = twin.param_meta(&program).unwrap();
    assert_eq!(
        (
            meta[0].name.as_str(),
            meta[0].offset_bytes,
            meta[0].default[0]
        ),
        ("rough", 48, 0.5)
    );
    assert_eq!(
        (
            meta[1].name.as_str(),
            meta[1].offset_bytes,
            meta[1].default[0]
        ),
        ("sun_yaw", 64, 0.25)
    );
    assert_eq!(meta[0].range.unwrap().0[0], 0.1);
    assert_eq!(meta[1].range.unwrap().1[0], 1.);
    assert!(
        program.modules[0]
            .origins
            .iter()
            .any(|origin| origin.span.0 == "edit.loft" && origin.span.1 == 5)
    );

    use Op::*;
    let tape = Tape::new(
        &allocate(&[
            Instruction {
                op: LoadInput,
                dst: 0,
                args: [0; 3],
                imm: 0,
            },
            Instruction {
                op: LoadRow,
                dst: 1,
                args: [0; 3],
                imm: 0,
            },
            Instruction {
                op: SdSphere,
                dst: 2,
                args: [0, 1, 0],
                imm: 0,
            },
            Instruction {
                op: Ret,
                dst: 0,
                args: [2, 0, 0],
                imm: 0,
            },
        ])
        .unwrap(),
    )
    .unwrap();
    for [x, y, z] in [
        [0., 0., 0.],
        [0.5, 0., 0.],
        [-1., 0.25, 0.125],
        [2., -3., 4.],
    ] {
        let value = tape
            .run(&Resources {
                constants: &[],
                row: &[[0.5; 4]],
                inputs: &[[x, y, z, 0.]],
                frame: &[],
                lut: &[],
                textures: &[],
            })
            .unwrap();
        let reference = twin.cpu_query(0, 0, [x, y, z], [1., 0., 0.]).unwrap();
        assert_close(value, [reference.trace; 4], "E+ trace");
    }
}
