//! Many instances in the game's own pass (Keel step 8), and the bench lane 4.1 scores: five instances in one pass against
//! five loads, and §4.3's trusted (unchecked) library modules against checked ones.
use super::*;
use loft_gpu::Instance;

const SIDE: u32 = 4;
// Pose: row i of r1 is a sphere (centre, radius) that moves with time; the instance's input x shifts the whole pose.
const POSE: &str = "@group(2) @binding(0) var<storage, read_write> w0: array<vec4<f32>>;
@compute @workgroup_size(64) fn pose(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= arrayLength(&w0)) { return; }
  let a = f32(id.x) * 0.39 + frame.time.x;
  w0[id.x] = vec4<f32>(frame.inputs[0].x + 0.3 * cos(a), 0.4 * sin(a * 1.3), 2.0 + 0.3 * sin(a), 0.08);
}";
// The subject: a quad around the instance, sphere-traced through the posed table with emit's clamped fetch.
const BOSS: &str = "@group(1) @binding(0) var<storage, read> r1: array<vec4<f32>>;
struct Out { @builtin(position) pos: vec4<f32>, @location(0) ndc: vec2<f32> }
@vertex fn vs(@builtin(vertex_index) i: u32) -> Out {
  var p = array<vec2<f32>, 6>(vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0));
  let q = vec2<f32>(frame.inputs[0].x, 0.0) + p[i] * vec2<f32>(0.45, 0.9);
  return Out(vec4<f32>(q, 0.5, 1.0), q);
}
fn fetch(row: f32) -> vec4<f32> { return r1[min(u32(row), arrayLength(&r1) - 1u)]; }
fn sdf(p: vec3<f32>) -> f32 {
  var d = 1e9;
  for (var i = 0u; i < SPHERES; i++) { let s = fetch(f32(i)); d = min(d, length(p - s.xyz) - s.w); }
  return d;
}
@fragment fn paint(o: Out) -> @location(0) vec4<f32> {
  let dir = normalize(vec3<f32>(o.ndc, 1.5));
  var t = 0.0;
  for (var k = 0u; k < 48u; k++) { let d = sdf(dir * t); if (d < 1e-3) { break; } t += d; }
  return vec4<f32>(vec3<f32>(exp(-t * 0.3)), 1.0);
}";
const SPHERES: u32 = 24;

/// A lamp-boss stand-in: a `Frame` pose kernel over `rows` and a sphere-traced subject reading it, at `origin`.
fn boss(origin: Origin) -> Bundle {
    let boss = BOSS.replace("SPHERES", &format!("{SPHERES}u"));
    Bundle {
        names: ["pose", "paint", "sky", "boss", "backdrop", "at"].map(Into::into).into(),
        resources: [
            Resource { shape: Shape::Frame, init: Init::Zero },
            Resource { shape: Shape::Rows { rows: SPHERES, lanes: 1 }, init: Init::Zero },
        ]
        .into(),
        packs: [pack(Access::Write, u64::from(SPHERES), &[(1, 0)])].into(),
        kernels: [Kernel {
            writes: [1].into(),
            reads: [].into(),
            body: Body::Map { module: 0, entry: 0, rows: SPHERES },
            when: When::Frame,
        }]
        .into(),
        modules: [module(0, origin), module(1, origin), module(2, origin)].into(),
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
        inputs: [InputDecl { name: 5, ty: loft_ir::graph::Ty::F32, def: [f(0.0)].into() }].into(),
        sections: text(&[POSE, &boss, SKY]),
        ..subject()
    }
}

fn target(d: &wgpu::Device, w: u32, h: u32) -> [wgpu::TextureView; 2] {
    [CANVAS.color, CANVAS.depth].map(|format| {
        d.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
        .create_view(&Default::default())
    })
}

/// The game's own frame: sky colour and far depth, as a game clears before drawing (untimed).
fn clear_game(enc: &mut wgpu::CommandEncoder, [c, z]: &[wgpu::TextureView; 2]) {
    enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: c,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::GREEN), store: wgpu::StoreOp::Store },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: z,
            depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
}

/// The game's pass over its colour and depth, loading what is there (the game drew its frame before).
fn game_pass<'a>(
    enc: &'a mut wgpu::CommandEncoder,
    [c, z]: &'a [wgpu::TextureView; 2],
    ts: Option<(&'a wgpu::QuerySet, u32)>,
) -> wgpu::RenderPass<'a> {
    let load = |_| wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store };
    enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: c,
            depth_slice: None,
            resolve_target: None,
            ops: load(()),
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: z,
            depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }),
            stencil_ops: None,
        }),
        timestamp_writes: ts.map(|(query_set, k)| wgpu::RenderPassTimestampWrites {
            query_set,
            beginning_of_pass_write_index: Some(k),
            end_of_pass_write_index: Some(k + 1),
        }),
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

fn instances(d: &wgpu::Device, q: &wgpu::Queue, loft: &Loft, xs: &[f32]) -> Vec<Instance> {
    xs.iter()
        .map(|&x| {
            let mut i = loft.instance(d, q).expect("instanced");
            i.set_input(loft, "at", &[x]).expect("input");
            i.time = 1.0;
            i
        })
        .collect()
}

#[test]
fn instances_draw_in_the_callers_pass_each_with_its_own_pose() {
    let (d, q) = device();
    let loft = load(&d, &q, &boss(Origin::Library));
    let insts = instances(&d, &q, &loft, &[-0.6, 0.6]);
    let game = Game::new(&d);
    let mut enc = d.create_command_encoder(&Default::default());
    game.clear(&mut enc, 1.0);
    loft.prepare(&q, &mut enc, &insts);
    let views = [game.color.create_view(&Default::default()), game.depth.create_view(&Default::default())];
    loft.draw_subject(&mut game_pass(&mut enc, &views, None), CANVAS, &insts).expect("drawn");
    let buf = d.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 256 * u64::from(N),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let layout = wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(256), rows_per_image: Some(N) };
    enc.copy_texture_to_buffer(
        game.color.as_image_copy(),
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout },
        game.color.size(),
    );
    q.submit([enc.finish()]);
    buf.map_async(wgpu::MapMode::Read, .., |r| r.expect("map"));
    d.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    let rgba: Vec<u8> =
        buf.get_mapped_range(..).expect("mapped").chunks(256).flat_map(|r| r[..(4 * N) as usize].to_vec()).collect();
    // Each quad sits at its own input: x = -0.6 covers columns 0-2 of 8, x = 0.6 columns 5-7; 3 and 4 stay the game's.
    let green = [0, 255, 0, 255];
    assert!(px(&rgba, 1, SIDE) != green && px(&rgba, 6, SIDE) != green, "both instances drew");
    assert_eq!(
        (px(&rgba, 3, SIDE), px(&rgba, 4, SIDE)),
        (green, green),
        "the game's pixels between them are untouched"
    );
    // A live stepped kernel cannot be shared by copies: refused by name.
    let live = load(&d, &q, &counter());
    assert!(matches!(live.instance(&d, &q), Err(Error::Usage(m)) if m.contains("steps live")));
}

/// Median of `f`'s GPU milliseconds over `n` frames.
fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// The resolved timestamps of `set` in ms, as (earliest begin, latest end) of its two passes (0/1 and 2/3). Passes
/// overlap on a tiled GPU, so a frame's GPU time is the span from its first begin to its last end, not a sum.
fn span(d: &wgpu::Device, q: &wgpu::Queue, set: &wgpu::QuerySet) -> (f64, f64) {
    let size = 8 * 4;
    let usage = wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC;
    let resolve = d.create_buffer(&wgpu::BufferDescriptor { label: None, size, usage, mapped_at_creation: false });
    let usage = wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ;
    let read = d.create_buffer(&wgpu::BufferDescriptor { label: None, size, usage, mapped_at_creation: false });
    // Metal: resolve only after the frame completed; a resolve in the next submit reads stale ends (crate docs).
    d.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    let mut enc = d.create_command_encoder(&Default::default());
    enc.resolve_query_set(set, 0..4, &resolve, 0);
    enc.copy_buffer_to_buffer(&resolve, 0, &read, 0, Some(size));
    q.submit([enc.finish()]);
    read.map_async(wgpu::MapMode::Read, .., |r| r.expect("map"));
    d.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    let period = f64::from(q.get_timestamp_period()) / 1e6;
    let t: Vec<f64> = read
        .get_mapped_range(..)
        .expect("mapped")
        .chunks(8)
        .map(|c| u64::from_le_bytes(c.try_into().expect("8 bytes")) as f64 * period)
        .collect();
    (t[0].min(t[2]), t[1].max(t[3]))
}

/// A frame's GPU span over every set it wrote.
fn frame_ms(spans: impl Iterator<Item = (f64, f64)>) -> f64 {
    let (a, b) = spans.fold((f64::MAX, f64::MIN), |(a, b), (x, y)| (a.min(x), b.max(y)));
    b - a
}

fn load1() -> String {
    std::process::Command::new("sysctl")
        .args(["-n", "vm.loadavg"])
        .output()
        .map_or("unknown".into(), |o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// `LOFT_BENCH_FRAMES=40 tools/dev.sh test loft-gpu --fast --run-ignored only -- five_instances --no-capture`
#[test]
#[ignore = "a GPU bench: run alone, outside the compile lock"]
fn five_instances_in_one_pass_against_five_loads_and_trusted_against_checked() {
    let frames: usize = std::env::var("LOFT_BENCH_FRAMES").ok().and_then(|s| s.parse().ok()).unwrap_or(40);
    let adapter =
        pollster::block_on(wgpu::Instance::default().request_adapter(&Default::default())).expect("an adapter");
    let required_features = wgpu::Features::TIMESTAMP_QUERY;
    let (d, q) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { required_features, ..Default::default() }))
            .expect("a device with timestamps");
    let (w, h) = (1280, 800);
    let xs = [-0.8, -0.4, 0.0, 0.4, 0.8];
    let views = &target(&d, w, h);
    let size = [w, h];
    let set =
        |n| d.create_query_set(&wgpu::QuerySetDescriptor { label: None, ty: wgpu::QueryType::Timestamp, count: n });
    println!("load1 at start: {}", load1());
    // Arm A: one load, five instances, one prepare pass and the subject in one game pass. Timestamps 0/1 and 2/3.
    let one = |origin| {
        let mut loft = load(&d, &q, &boss(origin));
        let qs = set(4);
        loft.set_timestamps(Some(qs.clone()));
        let insts = instances(&d, &q, &loft, &xs);
        move |d: &wgpu::Device, q: &wgpu::Queue| {
            let mut enc = d.create_command_encoder(&Default::default());
            clear_game(&mut enc, views);
            loft.prepare(q, &mut enc, &insts);
            loft.draw_subject(&mut game_pass(&mut enc, views, Some((&qs, 2))), CANVAS, &insts).expect("drawn");
            q.submit([enc.finish()]);
            frame_ms(std::iter::once(span(d, q, &qs)))
        }
    };
    let trusted = one(Origin::Library);
    let checked = one(Origin::Author);
    // Arm B: five loads, each its own kernel pass and render pass over the same game frame.
    let mut lofts: Vec<(Loft, wgpu::QuerySet)> = xs
        .iter()
        .map(|&x| {
            let mut l = load(&d, &q, &boss(Origin::Library));
            l.set_input("at", &[x]).expect("input");
            l.set_time(1.0);
            let qs = set(4);
            l.set_timestamps(Some(qs.clone()));
            (l, qs)
        })
        .collect();
    let mut five_loads = |d: &wgpu::Device, q: &wgpu::Queue| {
        let mut enc = d.create_command_encoder(&Default::default());
        clear_game(&mut enc, views);
        for (l, _) in lofts.iter_mut() {
            let view = View {
                canvas: CANVAS,
                color: &views[0],
                resolve: None,
                depth: &views[1],
                size,
                owner: Owner::Caller,
                layers: Layers::Subject,
            };
            l.encode_into(d, q, &mut enc, &view).expect("a frame");
        }
        q.submit([enc.finish()]);
        frame_ms(lofts.iter().map(|(_, qs)| span(d, q, qs)))
    };
    let (mut a, mut b, mut c) = (vec![], vec![], vec![]);
    for _ in 0..frames {
        a.push(trusted(&d, &q));
        b.push(five_loads(&d, &q));
        c.push(checked(&d, &q));
    }
    let (a, b, c) = (median(a), median(b), median(c));
    println!("five instances, one pass (trusted): {a:.3} ms GPU median of {frames}");
    println!("five loads, five passes (trusted):  {b:.3} ms");
    println!("five instances, one pass (checked): {c:.3} ms  (trusted/checked = {:.3})", a / c);
    println!("load1 at end: {}", load1());
    assert!(a < b, "five instances in one pass ({a:.3} ms) must cost less GPU than five loads ({b:.3} ms)");
}
