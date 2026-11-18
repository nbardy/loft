//! loft-gpu on a real device with hand-built bundles (lane 3.2's checks that do not need the compiler): a subject over a
//! game-owned colour and depth that clears nothing, layers, live steps with rewind, and the refusals.
use loft_gpu::{Canvas, Layers, Loft, Owner, View};
use loft_ir::Error;
use loft_ir::bundle::*;
use loft_ir::error::{LoadFault, SceneFault};
use loft_ir::identity::Precision;

mod instances;

const PRE: &str = "struct Frame { time: f32, params: array<vec4<f32>, 1>, inputs: array<vec4<f32>, 1> }
@group(0) @binding(0) var<uniform> frame: Frame;
";
const FILL: &str = "@group(2) @binding(0) var<storage, read_write> w0: array<vec4<f32>>;
@compute @workgroup_size(64) fn fill(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x < 1u) { w0[0] = vec4<f32>(f32(lf(1.0)), 0.0, 0.0, 1.0); }
}";
const SUBJECT: &str = "@group(1) @binding(0) var<storage, read> r1: array<vec4<f32>>;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
  var p = array<vec2<f32>, 6>(vec2(-0.5, -0.5), vec2(0.5, -0.5), vec2(0.5, 0.5), vec2(-0.5, -0.5), vec2(0.5, 0.5), vec2(-0.5, 0.5));
  return vec4<f32>(p[i], 0.5, 1.0);
}
@fragment fn paint() -> @location(0) vec4<f32> { return r1[0]; }";
const SKY: &str = "@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
  return vec4<f32>(f32(i == 1u) * 4.0 - 1.0, f32(i == 2u) * 4.0 - 1.0, 0.999, 1.0);
}
@fragment fn sky() -> @location(0) vec4<f32> { return vec4<f32>(0.0, 0.0, 1.0, 1.0); }";
// A counter stepped live: x counts steps, y is the time the last step saw.
const STEP: &str = "@group(1) @binding(0) var<storage, read> r2: array<vec4<f32>>;
@group(2) @binding(0) var<storage, read_write> w0: array<vec4<f32>>;
@compute @workgroup_size(64) fn step(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x < 1u) { w0[0] = vec4<f32>(r2[0].x + 1.0, frame.time, 0.0, 0.0); }
}";
const SHOW: &str = "@group(1) @binding(0) var<storage, read> r2: array<vec4<f32>>;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
  return vec4<f32>(f32(i == 1u) * 4.0 - 1.0, f32(i == 2u) * 4.0 - 1.0, 0.5, 1.0);
}
@fragment fn show() -> @location(0) vec4<f32> { return vec4<f32>(r2[0].x / 255.0, r2[0].y * 10.0 / 255.0, 0.0, 1.0); }";

fn f(x: f32) -> F32 {
    F32(x.to_bits())
}
fn module(wgsl: Id, reads: &[Id], origin: Origin) -> Module {
    Module { wgsl, origin, precision_alias: true, reads: reads.into() }
}
fn raster(depth_write: bool) -> Raster {
    Raster { cull: false, depth_write, blend: Blend::Opaque }
}
fn text(s: &[&str]) -> Box<[Box<[u8]>]> {
    s.iter().map(|t| format!("{PRE}{t}").into_bytes().into()).collect()
}

/// Sky (surroundings, a screen draw) then a red subject quad at depth 0.5 whose colour a load kernel fills.
fn subject() -> Bundle {
    Bundle {
        format: Format::V1,
        compiler: "test".into(),
        names: ["fill", "paint", "sky", "subject", "backdrop"].map(Into::into).into(),
        resources: [
            Resource { shape: Shape::Frame, init: Init::Zero },
            Resource { shape: Shape::Rows { rows: 1, lanes: 1 }, init: Init::By(0) },
        ]
        .into(),
        kernels: [Kernel {
            writes: [1].into(),
            reads: [].into(),
            body: Body::Map { module: 0, entry: 0, rows: 1 },
            when: When::Load,
        }]
        .into(),
        modules: [module(0, &[], Origin::Library), module(1, &[1], Origin::Library), module(2, &[], Origin::Library)]
            .into(),
        draws: [
            Draw { name: 4, layer: Layer::Surroundings, cover: Cover::Screen, shade: (2, 2), raster: raster(false) },
            Draw {
                name: 3,
                layer: Layer::Subject,
                cover: Cover::Raster { prim: RasterPrim::Grid { u: 1, v: 1 }, count: 1 },
                shade: (1, 1),
                raster: raster(true),
            },
        ]
        .into(),
        passes: [Pass {
            target: Target::View,
            camera: 0,
            draws: [0, 2],
            clear: Clear::Color([f(0.0), f(0.0), f(0.0), f(1.0)]),
            compose: Compose { outline: None, ground: None },
        }]
        .into(),
        media: [].into(),
        cameras: [].into(),
        params: [].into(),
        inputs: [].into(),
        sites: [].into(),
        sections: text(&[FILL, SUBJECT, SKY]),
    }
}

/// A `Latest` counter stepped every 0.5 s from 0 for 10 steps, shown full screen.
fn counter() -> Bundle {
    Bundle {
        names: ["step", "show", "counter"].map(Into::into).into(),
        resources: [
            Resource { shape: Shape::Frame, init: Init::Zero },
            Resource { shape: Shape::Rows { rows: 1, lanes: 1 }, init: Init::Zero },
            Resource { shape: Shape::Rows { rows: 1, lanes: 1 }, init: Init::Zero },
        ]
        .into(),
        kernels: [Kernel {
            writes: [2].into(),
            reads: [2].into(),
            body: Body::Map { module: 0, entry: 0, rows: 1 },
            when: When::Steps { dt: f(0.5), from: f(0.0), count: 10, keep: Keep::Latest },
        }]
        .into(),
        modules: [module(0, &[2], Origin::Library), module(1, &[2], Origin::Library)].into(),
        draws: [Draw { name: 2, layer: Layer::Subject, cover: Cover::Screen, shade: (1, 1), raster: raster(false) }]
            .into(),
        sections: text(&[STEP, SHOW]),
        passes: [Pass { draws: [0, 1], ..subject().passes[0].clone() }].into(),
        ..subject()
    }
}

fn device() -> (wgpu::Device, wgpu::Queue) {
    let adapter =
        pollster::block_on(wgpu::Instance::default().request_adapter(&Default::default())).expect("an adapter");
    let required_features = adapter.features() & wgpu::Features::SHADER_F16;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { required_features, ..Default::default() }))
        .expect("a device")
}

const N: u32 = 8;
const CANVAS: Canvas =
    Canvas { color: wgpu::TextureFormat::Rgba8Unorm, depth: wgpu::TextureFormat::Depth32Float, samples: 1 };

struct Game {
    color: wgpu::Texture,
    depth: wgpu::Texture,
}

impl Game {
    fn new(d: &wgpu::Device) -> Game {
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC;
        let tex = |format| {
            d.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width: N, height: N, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        Game { color: tex(CANVAS.color), depth: tex(CANVAS.depth) }
    }
    /// The game's own pass: its colour and depth before Loft draws.
    fn clear(&self, enc: &mut wgpu::CommandEncoder, depth: f32) {
        let view = self.color.create_view(&Default::default());
        let dview = self.depth.create_view(&Default::default());
        enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::GREEN), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &dview,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(depth), store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
    fn frame(
        &self,
        d: &wgpu::Device,
        q: &wgpu::Queue,
        loft: &mut Loft,
        depth: Option<f32>,
        owner: Owner,
        layers: Layers,
    ) -> (Vec<u8>, Vec<f32>) {
        let mut enc = d.create_command_encoder(&Default::default());
        if let Some(z) = depth {
            self.clear(&mut enc, z);
        }
        let (color, dview) = (self.color.create_view(&Default::default()), self.depth.create_view(&Default::default()));
        let view = View { canvas: CANVAS, color: &color, depth: &dview, size: [N, N], owner, layers };
        loft.encode_into(d, q, &mut enc, &view).expect("a frame");
        let read = |tex: &wgpu::Texture, enc: &mut wgpu::CommandEncoder| {
            let buf = d.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 256 * u64::from(N),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let layout = wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(256), rows_per_image: Some(N) };
            enc.copy_texture_to_buffer(
                tex.as_image_copy(),
                wgpu::TexelCopyBufferInfo { buffer: &buf, layout },
                tex.size(),
            );
            buf
        };
        let (cb, db) = (read(&self.color, &mut enc), read(&self.depth, &mut enc));
        q.submit([enc.finish()]);
        let bytes = |b: &wgpu::Buffer| {
            b.map_async(wgpu::MapMode::Read, .., |r| r.expect("map"));
            d.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
            b.get_mapped_range(..)
                .expect("mapped")
                .chunks(256)
                .flat_map(|row| row[..(4 * N) as usize].to_vec())
                .collect::<Vec<u8>>()
        };
        let depth = bytes(&db).chunks(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        (bytes(&cb), depth)
    }
}

fn px(rgba: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * N + x) * 4) as usize;
    [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
}

fn load(d: &wgpu::Device, q: &wgpu::Queue, b: &Bundle) -> Loft {
    pollster::block_on(Loft::load(d, q, &b.write().expect("writes"), Precision::Mixed, &[CANVAS], None)).expect("loads")
}

#[test]
fn subject_over_a_game_frame_loads_and_clears_nothing() {
    let (d, q) = device();
    let mut loft = load(&d, &q, &subject());
    let game = Game::new(&d);
    // Game depth far: the subject shows inside its quad and writes its depth there; outside, the game's pixels stay.
    let (rgba, depth) = game.frame(&d, &q, &mut loft, Some(1.0), Owner::Caller, Layers::Subject);
    assert_eq!(px(&rgba, 4, 4), [255, 0, 0, 255], "subject at the centre");
    assert_eq!(px(&rgba, 0, 0), [0, 255, 0, 255], "the game's colour in the corner: no clear, no sky");
    assert_eq!((depth[(4 * N + 4) as usize], depth[0]), (0.5, 1.0), "subject depth inside, the game's outside");
    // Game depth near: the game hides the subject everywhere and its depth is untouched.
    let (rgba, depth) = game.frame(&d, &q, &mut loft, Some(0.3), Owner::Caller, Layers::Subject);
    assert!(rgba.chunks(4).all(|p| p == [0, 255, 0, 255]), "the game's nearer depth hides the subject");
    assert!(depth.iter().all(|&z| z == 0.3));
    // Loft-owned: the pass clears, the sky covers the frame, the subject sits over it.
    let (rgba, _) = game.frame(&d, &q, &mut loft, None, Owner::Loft, Layers::All);
    assert_eq!((px(&rgba, 4, 4), px(&rgba, 0, 0)), ([255, 0, 0, 255], [0, 0, 255, 255]));
}

#[test]
fn latest_steps_follow_the_time_and_rewind() {
    let (d, q) = device();
    let mut loft = load(&d, &q, &counter());
    let game = Game::new(&d);
    // [steps run, time the last step saw · 10]: n = floor(t / 0.5), clamped to 10.
    for (t, want) in [(2.0, [4, 20]), (2.9, [5, 25]), (1.0, [2, 10]), (0.0, [0, 0]), (99.0, [10, 50]), (0.6, [1, 5])] {
        loft.set_time(t);
        let (rgba, _) = game.frame(&d, &q, &mut loft, Some(1.0), Owner::Caller, Layers::All);
        assert_eq!(&px(&rgba, 3, 3)[..2], &want, "t = {t}");
    }
}

#[test]
fn refusals_are_typed_and_named() {
    let (d, q) = device();
    let mut bytes = subject().write().expect("writes");
    bytes[4] = 2;
    let refused = pollster::block_on(Loft::load(&d, &q, &bytes, Precision::F32, &[CANVAS], None));
    assert!(matches!(refused, Err(Error::Load(LoadFault::FormatMismatch { found: 2, expected: 1, .. }))));
    // Author WGSL is the author's fault at its line, refused before any GPU object; a bad id is the bundle's.
    let mut typo = subject();
    typo.modules[1].origin = Origin::Author;
    typo.sections[1] = format!("{PRE}{}", SUBJECT.replace("r1[0];", "r1[0]")).into_bytes().into();
    let limits = d.limits();
    let got = loft_gpu::validate(&typo, Precision::F32, &limits);
    assert!(matches!(&got, Err(Error::Scene { fault: SceneFault::Wgsl { line: 8, .. }, .. })), "{got:?}");
    let mut stray = subject();
    stray.draws[1].shade = (7, 1);
    assert!(
        matches!(loft_gpu::validate(&stray, Precision::F32, &limits), Err(Error::Load(LoadFault::Invalid(m))) if m.contains("module 7"))
    );
    // A canvas the load did not build for is a usage error, not a validation panic.
    let mut loft = load(&d, &q, &subject());
    let game = Game::new(&d);
    let (c, z) = (game.color.create_view(&Default::default()), game.depth.create_view(&Default::default()));
    let other = Canvas { samples: 4, ..CANVAS };
    let view = View { canvas: other, color: &c, depth: &z, size: [N, N], owner: Owner::Caller, layers: Layers::All };
    let mut enc = d.create_command_encoder(&Default::default());
    assert!(matches!(loft.encode_into(&d, &q, &mut enc, &view), Err(Error::Usage(_))));
}

#[test]
fn load_kernels_see_param_defaults_and_a_3d_table_loads() {
    let (d, q) = device();
    // The load kernel paints the subject with param 0's default; a 3D table rides along unused.
    let mut b = subject();
    b.sections[0] = format!("{PRE}{}", FILL.replace("f32(lf(1.0))", "frame.params[0].x")).into_bytes().into();
    b.params = [ParamDecl { name: 0, def: f(0.5), min: f(0.0), max: f(1.0) }].into();
    b.resources = [
        &b.resources[..],
        &[Resource { shape: Shape::Texture { extent: [2, 2, 2], format: TexFormat::Rgba8Unorm }, init: Init::Zero }],
    ]
    .concat()
    .into();
    let mut loft = load(&d, &q, &b);
    let (rgba, _) = Game::new(&d).frame(&d, &q, &mut loft, Some(1.0), Owner::Caller, Layers::Subject);
    assert_eq!(px(&rgba, 4, 4)[0], 128, "the load kernel read param 0's default, 0.5");
}
