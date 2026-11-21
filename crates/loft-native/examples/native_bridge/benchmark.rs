use loft_ir::*;
use loft_native::{Gpu, metal_fx::TemporalScaler};
use loft_runner::{
    Runner,
    frame::{FrameOptions, HostCallback},
};
use std::{fs, path::Path};
use wgpu::{BufferUsages as B, TextureFormat as F, TextureUsages as T};

#[path = "slots.rs"]
mod slots;
use slots::{attach, texture};

const SIDE: usize = 512;
const FULL: [u32; 2] = [1280, 800];
const LOW: [u32; 2] = [640, 400];
const WARMUP: usize = 24;
const FRAMES: usize = 64;

struct Timer {
    set: wgpu::QuerySet,
    buffer: wgpu::Buffer,
    count: u32,
}
impl Timer {
    fn new(g: &Gpu, count: u32) -> Self {
        let set = g.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: None,
            count,
            ty: wgpu::QueryType::Timestamp,
        });
        let buffer = g.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(count) * 8,
            usage: B::QUERY_RESOLVE | B::COPY_SRC,
            mapped_at_creation: false,
        });
        Self { set, buffer, count }
    }
    fn read(&self, g: &Gpu) -> f64 {
        g.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let mut resolve = g.device.create_command_encoder(&Default::default());
        resolve.resolve_query_set(&self.set, 0..self.count, &self.buffer, 0);
        g.queue.submit([resolve.finish()]);
        let bytes = loft_runner::read_buffer(&g.device, &g.queue, &self.buffer).unwrap();
        let ticks = bytes
            .as_chunks::<8>()
            .0
            .iter()
            .map(|b| u64::from_le_bytes(*b))
            .collect::<Vec<_>>();
        let elapsed = ticks[self.count as usize - 1]
            .checked_sub(ticks[0])
            .unwrap();
        assert!(ticks[0] > 0 && elapsed > 0, "{ticks:?}");
        elapsed as f64 * f64::from(g.queue.get_timestamp_period()) / 1e6
    }
}

fn compare(g: &Gpu, names: [&str; 2], mut frame: impl FnMut(usize, usize, &Timer), count: u32) {
    let timers = [Timer::new(g, count), Timer::new(g, count)];
    let mut times = [Vec::new(), Vec::new()];
    for i in 0..WARMUP + FRAMES {
        for j in [i % 2, 1 - i % 2] {
            frame(j, i, &timers[j]);
            if i >= WARMUP {
                times[j].push(timers[j].read(g));
            } else {
                g.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            }
        }
    }
    let stats = |ms: &mut [f64]| {
        ms.sort_by(f64::total_cmp);
        [ms[FRAMES / 2 - 1], ms[(FRAMES * 95).div_ceil(100) - 1]]
    };
    let mut delta: Vec<_> = times[1].iter().zip(&times[0]).map(|(b, a)| b - a).collect();
    for (name, ms) in names.into_iter().zip(&mut times) {
        let [p50, p95] = stats(ms);
        println!("{name}: p50 {p50:.6} ms, p95 {p95:.6} ms");
    }
    println!(
        "paired native minus control p50 {:+.6} ms",
        stats(&mut delta)[0]
    );
}

fn field(g: &Gpu, out: &Path) {
    let src = slots::scene("field.loft");
    let (program, _, c) =
        loft_passes::compile(&src, &[]).unwrap_or_else(|err| panic!("{}", err.render(&src)));
    let bundle = loft_passes::bundle::lower(&program, &c, (512, 512), |_, _| vec![0; 256]).unwrap();
    let target =
        texture(g, [1; 2], F::Rgba8Unorm, T::RENDER_ATTACHMENT).create_view(&Default::default());
    let mut features = Vec::with_capacity(SIDE * SIDE * 8);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let p = |i| (i as f32 + 0.5) / SIDE as f32 * 2.0 - 1.0;
            let (x, y) = (p(x), p(y));
            features.extend([x, y, 0.19, x * y, x * x, y * y, (4.0 * x).sin(), 1.0]);
        }
    }
    let mut w = [0.0f32; 256];
    // The first six ReLUs recover |x|+|y|+|z|. The other units encode directional radiance.
    for k in 0..3 {
        w[16 * k + 2 * k] = 1.0;
        w[16 * k + 2 * k + 1] = -1.0;
    }
    for h in 6..15 {
        for k in 0..8 {
            w[16 * k + h] = ((h * 13 + k * 17) % 23) as f32 / 23.0 - 0.5;
        }
        for k in 0..3 {
            w[128 + 8 * h + k] = ((h * 7 + k * 11) % 17) as f32 / 32.0;
        }
    }
    for h in 0..6 {
        w[128 + 8 * h + 3] = 1.0;
    }
    w[16 * 7 + 15] = 1.0;
    w[248..252].copy_from_slice(&[0.04, 0.10, 0.20, -0.72]);
    let pack = |f: &[f32]| f.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>();
    let mut routes = [HostCaps::WEB_BASELINE, g.host_caps()].map(|caps| {
        let mut r = attach(g, caps);
        let id = r.load_bundle(&bundle).unwrap();
        for (name, data) in [("Features", &features[..]), ("Weights", &w[..])] {
            g.queue.write_buffer(
                &r.export_buffer(id, c.compute.tables[name]).unwrap(),
                0,
                &pack(data),
            );
        }
        (r, id)
    });
    compare(
        g,
        ["neural ALU 512²", "neural cooperative f32 512²"],
        |j, _, timer| {
            let (r, id) = &mut routes[j];
            let mut enc = g.device.create_command_encoder(&Default::default());
            r.encode_frame(
                *id,
                &mut enc,
                &target,
                0.,
                FrameOptions {
                    timestamps: Some(&timer.set),
                    ..Default::default()
                },
            )
            .unwrap();
            g.queue.submit([enc.finish()]);
        },
        2,
    );
    for (index, (runner, id)) in routes.iter().enumerate() {
        let image = runner
            .export_buffer(*id, c.compute.tables["Image"])
            .unwrap();
        let bytes = loft_runner::read_buffer(&g.device, &g.queue, &image).unwrap();
        let mut rgb = Vec::new();
        let mut err = 0.0f64;
        for (input, output) in features.chunks_exact(8).zip(bytes.as_chunks::<16>().0) {
            let expected = |k| {
                (0..16)
                    .map(|h| {
                        let v: f64 = (0..8)
                            .map(|i| f64::from(input[i]) * f64::from(w[i * 16 + h]))
                            .sum();
                        v.max(0.0) * f64::from(w[128 + h * 8 + k])
                    })
                    .sum::<f64>()
            };
            let values = std::array::from_fn::<_, 4, _>(|i| {
                f32::from_le_bytes(output[i * 4..i * 4 + 4].try_into().unwrap())
            });
            for (k, value) in values.into_iter().enumerate() {
                assert!(value.is_finite());
                err = err.max((f64::from(value) - expected(k)).abs());
            }
            let color = if values[3] <= 0. {
                [values[0], values[1], values[2]]
            } else {
                [0.035, 0.05, 0.08]
            };
            rgb.extend(color.map(|v| (v.clamp(0., 1.).sqrt() * 255.0).round() as u8));
        }
        assert!(err < 2e-6, "field route {index}: max abs {err:e}");
        println!(
            "field route {index}: {} CPU comparisons, max abs {err:e}",
            SIDE * SIDE * 4
        );
        if index == 1 {
            ppm(out.join("field.ppm"), [SIDE as u32; 2], &rgb);
        }
    }
}

const COLOR: TableId = TableId(0);
const DEPTH: TableId = TableId(1);
const OUT: TableId = TableId(2);
const DRAW: TableId = TableId(3);
const FRAME: TableId = TableId(4);
const UPSCALE: PassId = PassId(1);
const PRESENT: TableId = TableId(5);

fn scene_bundle(input: [u32; 2]) -> Bundle {
    let table = |id, initial: Vec<u8>| TableDecl {
        id,
        stride_bytes: 4,
        capacity: (initial.len() / 4) as u32,
        format: ElemFormat::U32,
        lifetime: if initial.is_empty() {
            TableLifetime::External
        } else {
            TableLifetime::Static
        },
        initial: initial.into_boxed_slice(),
    };
    let attachment = |table, format, [width, height]: [u32; 2]| TextureDecl {
        table,
        format,
        extent: [width, height, 1],
        samples: 1,
        clear: Some(if table == DEPTH {
            [1.0, 0.0, 0.0, 0.0]
        } else {
            [0.0; 4]
        }),
    };
    let pass = |id: PassId, bindings: Vec<PassBinding>, run| PassDecl {
        id,
        wave: id.0 as u16,
        queue: QueueClass::Graphics,
        bindings: bindings.into_boxed_slice(),
        run,
    };
    let bind = |slot, resource, access: BindAccess| PassBinding {
        group: access.group(),
        slot,
        resource,
        access,
    };
    let module = |id, src: &str| ModuleBlob {
        id: ModuleId(id),
        hash: *blake3::hash(src.as_bytes()).as_bytes(),
        target: ShaderTarget::Wgsl,
        bytes: src.as_bytes().into(),
    };
    let dispatch = DispatchExtent::Fixed([FULL[0] / 8, FULL[1] / 8, 1]);
    Bundle {
        resources: Resources {
            frame: Some(FrameData {
                main: FRAME,
                time_offset: 16,
                variants: vec![],
            }),
            textures: vec![
                attachment(COLOR, ImageFormat::Rgba16Float, input),
                attachment(DEPTH, ImageFormat::Depth32Float, input),
                attachment(OUT, ImageFormat::Rgba16Float, FULL),
                attachment(PRESENT, ImageFormat::Rgba16Float, FULL),
            ],
            ..Default::default()
        },
        modules: vec![
            module(0, include_str!("scene.wgsl")),
            module(1, include_str!("finish.wgsl")),
        ],
        tables: vec![
            table(COLOR, vec![]),
            table(DEPTH, vec![]),
            table(OUT, vec![]),
            table(PRESENT, vec![]),
            table(
                DRAW,
                [3u32, 1, 0, 0]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect(),
            ),
            table(FRAME, vec![0; 256]),
        ],
        passes: vec![
            pass(
                PassId(0),
                vec![],
                PassRun::RenderBundle {
                    draws: Box::new([IndirectDraw {
                        module: ModuleId(0),
                        vertex_entry: "vs".into(),
                        fragment_entry: "fs".into(),
                        args: DRAW,
                        offset_bytes: 0,
                        vertex: None,
                        index: None,
                    }]),
                    color_targets: Box::new([TargetDesc { table: COLOR }]),
                    depth: Some(DepthDesc {
                        table: DEPTH,
                        write: true,
                    }),
                    msaa: 1,
                },
            ),
            pass(
                UPSCALE,
                vec![
                    bind(0, COLOR, BindAccess::SampledTex),
                    bind(1, OUT, BindAccess::StorageTex),
                ],
                PassRun::HostCallback {
                    module: ModuleId(1),
                    entry: "upscale".into(),
                    dispatch: dispatch.clone(),
                },
            ),
            pass(
                PassId(2),
                vec![
                    bind(0, OUT, BindAccess::SampledTex),
                    bind(1, PRESENT, BindAccess::StorageTex),
                ],
                PassRun::Compute {
                    module: ModuleId(1),
                    entry: "upscale".into(),
                    dispatch,
                },
            ),
        ],
        ..Default::default()
    }
}

struct Scene {
    runner: Runner,
    id: InstanceId,
    input: [u32; 2],
}
impl Scene {
    fn frame(
        &mut self,
        g: &Gpu,
        view: &wgpu::TextureView,
        timer: Option<&Timer>,
        jitter: [f32; 2],
        effect: Option<(&mut TemporalScaler, bool)>,
    ) {
        let mut enc = g.device.create_command_encoder(&Default::default());
        let mut prefix = Vec::new();
        let mut raw = effect.map(|(fx, reset)| {
            move || {
                // Both scene inputs and output were initialized through wgpu; submit in graph order below.
                #[allow(unsafe_code)]
                unsafe {
                    fx.record(jitter, false, reset)
                }
            }
        });
        let host_callback = raw.as_mut().map(|record| HostCallback {
            pass: UPSCALE,
            prefix: &mut prefix,
            record,
        });
        let uniform = |_| {
            let [w, h] = self.input.map(|n| 1.0 / n as f32);
            let mut frame = vec![0; 256];
            for (word, value) in frame.chunks_exact_mut(4).zip([w, h, jitter[0], jitter[1]]) {
                word.copy_from_slice(&value.to_le_bytes());
            }
            frame
        };
        let record = self
            .runner
            .encode_frame(
                self.id,
                &mut enc,
                view,
                0.,
                FrameOptions {
                    timestamps: timer.map(|t| &t.set),
                    uniform: Some(&uniform),
                    host_callback,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(record.commands, 3);
        prefix.push(enc.finish());
        g.queue.submit(prefix);
    }

    fn pixels(&self, g: &Gpu) -> Vec<u8> {
        // Reading the downstream wgpu pass ensures MetalFX pixels crossed back into the graph.
        let tex = self.runner.export_texture(self.id, PRESENT).unwrap();
        let bytes = loft_runner::read_texture(&g.device, &g.queue, &tex, 0).unwrap();
        bytes
            .chunks_exact(8)
            .flat_map(|p| {
                std::array::from_fn::<_, 3, _>(|i| {
                    let n = half::f16::from_le_bytes([p[2 * i], p[2 * i + 1]]).to_f32();
                    (n.clamp(0., 1.).sqrt() * 255.0).round() as u8
                })
            })
            .collect()
    }
}

fn ppm(path: impl AsRef<Path>, [w, h]: [u32; 2], rgb: &[u8]) {
    let mut bytes = format!("P6\n{w} {h}\n255\n").into_bytes();
    bytes.extend(rgb);
    fs::write(path, bytes).unwrap();
}

fn upscaler(g: &Gpu, out: &Path) {
    let [mut full, mut low] = [FULL, LOW].map(|input| {
        let mut runner = attach(g, g.host_caps());
        let id = runner.load_bundle(&scene_bundle(input)).unwrap();
        Scene { runner, id, input }
    });
    let view =
        texture(g, FULL, F::Rgba8Unorm, T::RENDER_ATTACHMENT).create_view(&Default::default());
    let motion = texture(
        g,
        LOW,
        F::Rg16Float,
        T::RENDER_ATTACHMENT | T::TEXTURE_BINDING,
    );
    let mut init = g.device.create_command_encoder(&Default::default());
    init.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &motion.create_view(&Default::default()),
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    g.queue.submit([init.finish()]);
    low.frame(g, &view, None, [0.; 2], None);
    let [color, depth, output] =
        [COLOR, DEPTH, OUT].map(|t| low.runner.export_texture(low.id, t).unwrap());
    let mut effect = TemporalScaler::new(&g.device, &color, &depth, &motion, &output).unwrap();
    let bilinear = low.pixels(g);
    let halton = |mut n, base| {
        let (mut value, mut step) = (0.0, 1.0);
        while n != 0 {
            step /= base as f32;
            value += (n % base) as f32 * step;
            n /= base;
        }
        value - 0.5
    };
    compare(
        g,
        ["full 1280×800", "temporal MetalFX 640×400 → 1280×800"],
        |route, serial, timer| {
            if route == 0 {
                full.frame(g, &view, Some(timer), [0.; 2], None);
            } else {
                let i = serial % 32 + 1;
                low.frame(
                    g,
                    &view,
                    Some(timer),
                    [halton(i, 2), halton(i, 3)],
                    Some((&mut effect, serial == 0)),
                );
            }
        },
        6,
    );
    let reference = full.pixels(g);
    let upscale = low.pixels(g);
    let psnr = |image: &[u8]| {
        let mse = reference
            .iter()
            .zip(image)
            .map(|(&a, &b)| (f64::from(a) - f64::from(b)).powi(2))
            .sum::<f64>()
            / image.len() as f64;
        10.0 * (255.0f64.powi(2) / mse).log10()
    };
    let (fx, linear) = (psnr(&upscale), psnr(&bilinear));
    println!("display RGB PSNR vs full resolution: MetalFX {fx:.3} dB, bilinear {linear:.3} dB");
    ppm(out.join("full.ppm"), FULL, &reference);
    ppm(out.join("bilinear.ppm"), FULL, &bilinear);
    ppm(out.join("metalfx.ppm"), FULL, &upscale);
    assert!(fx >= 30.0 && upscale != bilinear && upscale != reference);
}

pub fn run() {
    let g = loft_native::gpu().unwrap();
    let out = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "out/native_bridge".into());
    fs::create_dir_all(&out).unwrap();
    let caps = g.host_caps().features;
    assert!(g.timestamps && caps.contains(GpuFeatures::COOP_MATRIX | GpuFeatures::METAL_FX));
    println!(
        "{} {caps:?}; {WARMUP} warmups, {FRAMES} alternating pairs",
        g.adapter
    );
    println!(
        "triangle before p50/p95 {:.6?} ms",
        g.triangle_control((FULL[0], FULL[1])).unwrap()
    );
    slots::check(&g);
    field(&g, &out);
    upscaler(&g, &out);
    println!(
        "triangle after p50/p95 {:.6?} ms",
        g.triangle_control((FULL[0], FULL[1])).unwrap()
    );
}
