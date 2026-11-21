use loft_format::bundle::{BundleError, decode, encode, validate_wgsl};
use loft_ir::*;
use std::collections::BTreeMap;

#[test]
fn container_rejects_corruption() {
    let module = b"@compute @workgroup_size(1) fn main() {}";
    let bundle = Bundle {
        modules: vec![ModuleBlob {
            id: ModuleId(4),
            hash: *blake3::hash(module).as_bytes(),
            target: ShaderTarget::Wgsl,
            bytes: module.as_slice().into(),
        }],
        tables: vec![TableDecl {
            id: TableId(7),
            format: ElemFormat::F32,
            lifetime: TableLifetime::DynamicArena { max_rows: 5 },
            stride_bytes: 4,
            capacity: 2,
            initial: [0x00, 0x00, 0x00, 0x80].as_slice().into(),
        }],
        rows: BTreeMap::from([(KindId(3), TableId(7))]),
        passes: vec![PassDecl {
            id: PassId(2),
            wave: 1,
            queue: QueueClass::Graphics,
            bindings: Box::new([]),
            run: PassRun::Compute {
                module: ModuleId(4),
                entry: "main".into(),
                dispatch: DispatchExtent::Fixed([1, 1, 1]),
            },
        }],
        ..Bundle::default()
    };
    let mut encoded = encode(&bundle);
    let decoded = decode(&encoded).unwrap();
    validate_wgsl(&decoded).unwrap();
    assert_eq!(encode(&decoded), encoded);
    assert_eq!(decoded.tables[0].initial.as_ref(), &[0, 0, 0, 0x80]);
    assert_eq!(
        decode(&encoded[..encoded.len() - 1]).unwrap_err(),
        BundleError::Truncated
    );
    *encoded.last_mut().unwrap() ^= 1;
    assert_eq!(decode(&encoded).unwrap_err(), BundleError::Hash);
}

#[test]
fn pass_tree_preserves_the_wire_depth_limit() {
    let pass = |run| PassDecl {
        id: PassId(0),
        wave: 0,
        queue: QueueClass::Graphics,
        bindings: Box::new([]),
        run,
    };
    let mut nested = pass(PassRun::GenerateMips {
        texture: TableId(1),
    });
    for depth in 1..=33 {
        let bundle = Bundle {
            passes: vec![nested.clone()],
            ..Bundle::default()
        };
        let bytes = encode(&bundle);
        if depth <= 32 {
            assert_eq!(encode(&decode(&bytes).unwrap()), bytes);
        } else {
            assert_eq!(decode(&bytes).unwrap_err(), BundleError::Encoding);
        }
        nested = pass(PassRun::PassLoop {
            max_iters: 2,
            cond_buf: None,
            body: Box::new([nested]),
        });
    }
}
