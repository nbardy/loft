//! Layout shared by the execution declarations and the LOFTB001 codec.

/// Apply the frozen schema to the IR declarations or the wire codec.
/// `/` starts a field stored in a separate bulk section. `nested` guards recursive decoding.
#[doc(hidden)]
#[macro_export]
macro_rules! bundle_schema {
    ($ids:ident, $record:ident, $sum:ident) => {
        $ids!(ModuleId, TableId, PassId, KindId, ParamId);

        $sum!(#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        ShaderTarget {
            0 => NagaIr,
            1 => Wgsl,
            2 => Msl,
            3 => Spirv,
            4 => Ptx,
            5 => Wasm,
        });

        $sum!(#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        QueueClass {
            0 => Graphics,
            1 => AsyncCompute,
            2 => Transfer,
        });

        $sum!(#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        ElemFormat {
            0 => F32, 1 => F16, 2 => Bf16, 3 => F64, 4 => U32, 5 => I32, 6 => U16, 7 => I16,
            8 => U8, 9 => I8, 10 => PackedU4, 11 => PackedI4, 12 => PackedU2, 13 => Fp8E4M3,
            14 => Fp8E5M2, 15 => Struct(value: u32),
        });

        $sum!(#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        BindAccess {
            0 => ReadStorage,
            1 => ReadWriteStorage,
            2 => SampledTex,
            3 => StorageTex,
            4 => BindlessTexArray,
            5 => Sampler,
        });

        $sum!(#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        IrTy {
            0 => F32,
            1 => Vec2,
            2 => Vec3,
            3 => Vec4,
            4 => U32,
            5 => Bool,
        });

        $sum!(#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        TableLifetime {
            0 => Static,
            1 => DynamicArena { max_rows: u32 },
            2 => Transient,
            3 => PingPong,
            4 => External,
        });

        $record!(#[derive(Clone, Debug)]
        TableDecl {
            id: TableId,
            format: ElemFormat,
            lifetime: TableLifetime,
            /// Addressable rows consist of one or more whole storage words, including packed lanes.
            stride_bytes: u32,
            capacity: u32,
        } / initial: Box<[u8]>);

        $record!(#[derive(Clone, Debug)]
        StructLayout {
            stride_bytes: u32,
            offsets: Box<[u32]>,
        });

        $record!(#[derive(Clone, Debug)]
        ModuleBlob {
            id: ModuleId,
            hash: [u8; 32],
            target: ShaderTarget,
        } / bytes: Box<[u8]>);

        $record!(#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
        PassBinding {
            group: u8,
            slot: u32,
            resource: TableId,
            access: BindAccess,
        });

        $sum!(#[derive(Clone, Debug)]
        DispatchExtent {
            0 => Fixed(value: [u32; 3]),
            1 => FromRowCount { table: TableId, workgroup_x: u32 },
            2 => Indirect { buf: TableId, offset_bytes: u32 },
        });

        $record!(#[derive(Clone, Debug)]
        IndirectDraw {
            module: ModuleId,
            vertex_entry: String,
            fragment_entry: String,
            args: TableId,
            offset_bytes: u32,
            vertex: Option<TableId>,
            index: Option<TableId>,
        });

        $record!(#[derive(Clone, Debug)]
        TargetDesc {
            table: TableId,
        });

        $record!(#[derive(Clone, Debug)]
        DepthDesc {
            table: TableId,
            write: bool,
        });

        $sum!(#[derive(Clone, Debug)]
        PassRun {
            0 => Compute {
                module: ModuleId,
                entry: String,
                dispatch: DispatchExtent,
            },
            1 => ComputeFusedTicks {
                module: ModuleId,
                entry: String,
                max_ticks: u32,
                instances: TableId,
            },
            2 => RenderBundle {
                draws: Box<[IndirectDraw]>,
                color_targets: Box<[TargetDesc]>,
                depth: Option<DepthDesc>,
                msaa: u8,
            },
            3 => PassLoop {
                max_iters: u32,
                cond_buf: Option<(TableId, u32)>,
                body: Box<[PassDecl]>,
            },
            4 => GenerateMips {
                texture: TableId,
            },
        });

        $record!(#[derive(Clone, Debug)]
        PassDecl {
            id: PassId,
            wave: u16,
            queue: QueueClass,
            bindings: Box<[PassBinding]>,
            run: PassRun,
        } nested);

        $record!(#[derive(Clone, Debug)]
        StateStoreDecl {
            table: TableId,
            layout_hash: [u8; 32],
            checkpoint_slots: u32,
        });

        $record!(#[derive(Clone, Debug)]
        ParamMeta {
            id: ParamId,
            name: String,
            ty: IrTy,
            offset_bytes: u32,
            default: [f32; 4],
            range: Option<([f32; 4], [f32; 4])>,
        });

        $sum!(#[derive(Clone, Copy, Debug)]
        ImageFormat { 0 => Rg32Uint, 1 => Rgba16Float, 2 => Depth32Float });

        $record!(#[derive(Clone, Copy, Debug)]
        TextureDecl {
            table: TableId,
            format: ImageFormat,
            extent: [u32; 3],
            samples: u8,
            clear: Option<[f64; 4]>,
        });

        $record!(#[derive(Clone, Debug)]
        FrameData {
            main: TableId,
            variants: Vec<(PassId, TableId)>,
            time_offset: u32,
        });

        $sum!(#[derive(Clone, Copy, Debug, PartialEq, Eq)]
        InputKind { 0 => Level, 1 => Event, 2 => Packet });

        $record!(#[derive(Clone, Copy, Debug)]
        InputSlot {
            kind: InputKind,
            offset_words: u32,
            words: u32,
        });

        $record!(#[derive(Clone, Debug, Default)]
        Resources {
            inputs: Vec<InputSlot>,
            textures: Vec<TextureDecl>,
            comparison_samplers: Vec<TableId>,
            resets: Vec<TableId>,
            cull_front: Vec<PassId>,
            frame: Option<FrameData>,
        });
    };
}
