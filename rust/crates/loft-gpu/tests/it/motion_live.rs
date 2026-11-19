//! Opt-in score on actual emitted bundles. Run tools/motion_fixtures.ts first; see the implementation note.
use super::*;
use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

struct Run {
    d: wgpu::Device,
    q: wgpu::Queue,
    dir: PathBuf,
    targets: [wgpu::TextureView; 2],
    queries: wgpu::QuerySet,
}
const SIZE: [u32; 2] = [256, 192];
const TICKS: u32 = 480;

impl Run {
    fn buffer(&self, size: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
        self.d.create_buffer(&wgpu::BufferDescriptor { label: None, size, usage, mapped_at_creation: false })
    }
    fn stage(&self, size: u64) -> wgpu::Buffer {
        self.buffer(size, wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST)
    }
    fn bytes(&self, b: &wgpu::Buffer) -> Vec<u8> {
        b.map_async(wgpu::MapMode::Read, .., |r| r.expect("map"));
        self.d.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let result = b.get_mapped_range(..).unwrap().to_vec();
        b.unmap();
        result
    }
    fn read(&self, name: &str) -> Bundle {
        Bundle::read(&fs::read(self.dir.join(format!("{name}.loft"))).unwrap()).unwrap()
    }
    fn load(&self, b: &Bundle) -> Loft {
        let bytes = b.write().unwrap();
        let t = Instant::now();
        let l = pollster::block_on(Loft::load(&self.d, &self.q, &bytes, Precision::F32, &[CANVAS], None)).unwrap();
        self.d.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        println!("load {:.3} ms; built {:?}", t.elapsed().as_secs_f64() * 1000.0, l.built());
        l
    }
    fn encode(&self, l: &mut Loft, enc: &mut wgpu::CommandEncoder) {
        l.encode_into(
            &self.d,
            &self.q,
            enc,
            &View {
                canvas: CANVAS,
                color: &self.targets[0],
                depth: &self.targets[1],
                resolve: None,
                owner: Owner::Loft,
                layers: Layers::All,
                size: SIZE,
            },
        )
        .unwrap();
    }
    fn spans(&self) -> [f64; 4] {
        self.d.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let mut e = self.d.create_command_encoder(&Default::default());
        let dst = self.stage(32);
        let r = self.buffer(32, wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC);
        e.resolve_query_set(&self.queries, 0..4, &r, 0);
        e.copy_buffer_to_buffer(&r, 0, &dst, 0, None);
        self.q.submit([e.finish()]);
        let bytes = self.bytes(&dst);
        let ns = self.q.get_timestamp_period() as f64;
        std::array::from_fn(|i| u64::from_le_bytes(bytes[8 * i..8 * i + 8].try_into().unwrap()) as f64 * ns / 1e6)
    }
    fn trace(&self, name: &str) -> Vec<u8> {
        let b = self.read(name);
        let mut l = self.load(&b);
        let output = b.row_steps[0].output;
        let states = self.stage(481 * 672);
        let poses = self.stage(481 * 928);
        let mut e = self.d.create_command_encoder(&Default::default());
        for k in 0..=TICKS {
            l.set_time(k as f32 / 120.0);
            assert_eq!(l.advance_rows(&mut e), 1);
            l.copy_state(&mut e, 0, Some(k), &states, u64::from(k) * 672).unwrap();
            l.copy_rows(&mut e, output, &poses, u64::from(k) * 928).unwrap();
        }
        self.q.submit([e.finish()]);
        let states = self.bytes(&states);
        fs::write(self.dir.join(format!("{name}.state")), &states).unwrap();
        fs::write(self.dir.join(format!("{name}.pose")), self.bytes(&poses)).unwrap();
        if name == "one" {
            let mut e = self.d.create_command_encoder(&Default::default());
            for k in 1..=TICKS {
                // One representable instant just to the left of the next tick.
                l.set_time(f32::from_bits((k as f32 / 120.).to_bits() - 1));
                l.advance_rows(&mut e);
                l.copy_rows(&mut e, output, &poses, u64::from(k) * 928).unwrap();
            }
            self.q.submit([e.finish()]);
            fs::write(self.dir.join("left.pose"), self.bytes(&poses)).unwrap();
        }
        states
    }
    fn replay(&self, group: u32, alternate: bool, baked: &[u8]) -> (Vec<u8>, loft_ir::identity::FrameKey, Vec<u8>) {
        let mut l = self.load(&self.read("one"));
        let mut k = 0;
        let mut frames = 0;
        let mut dispatches = 0;
        while k < TICKS {
            let n = if group == 6 { if k == 202 { 6 } else { 2 } } else { group };
            let end = TICKS.min(k + n);
            for j in k + 1..=end {
                let z = match j {
                    24..120 if alternate => 0.0,
                    24..204 => 1.2,
                    204..384 => -2.1,
                    _ => 0.0,
                };
                l.write_tick("stick", 0, j, &[0.0, z], None).unwrap();
            }
            let mut e = self.d.create_command_encoder(&Default::default());
            l.set_time(end as f32 / 120.0);
            dispatches += l.advance_rows(&mut e);
            frames += 1;
            self.q.submit([e.finish()]);
            k = end;
        }
        let states = self.stage(481 * 672);
        let mut e = self.d.create_command_encoder(&Default::default());
        for j in 0..=TICKS {
            l.copy_state(&mut e, 0, Some(j), &states, 672 * u64::from(j)).unwrap();
        }
        self.q.submit([e.finish()]);
        let bytes = self.bytes(&states);
        let diff = bytes.chunks_exact(672).zip(baked.chunks_exact(672)).filter(|(a, b)| a != b).count();
        println!(
            "group {group} alternate {alternate}: {diff}/481 states differ, {dispatches}/{frames} dispatches/frames"
        );
        assert_eq!(dispatches, frames);
        if !alternate {
            assert_eq!(diff, 0, "regrouping changed a state");
        }
        l.use_camera("game").unwrap();
        let key = l.frame_key(0, SIZE);
        let rgba = self.picture(&mut l);
        if !alternate && group == 4 {
            // Rewind off a checkpoint, then resume from it.
            let mut e = self.d.create_command_encoder(&Default::default());
            l.set_time(177.0 / 120.0);
            l.advance_rows(&mut e);
            let dst = self.stage(672);
            l.copy_state(&mut e, 0, None, &dst, 0).unwrap();
            self.q.submit([e.finish()]);
            assert_eq!(self.bytes(&dst), &baked[177 * 672..178 * 672]);
        }
        (bytes, key, rgba)
    }
    fn picture(&self, l: &mut Loft) -> Vec<u8> {
        let mut e = self.d.create_command_encoder(&Default::default());
        self.encode(l, &mut e);
        let size = wgpu::Extent3d { width: SIZE[0], height: SIZE[1], depth_or_array_layers: 1 };
        let dst = self.stage(u64::from(4 * SIZE[0] * SIZE[1]));
        e.copy_texture_to_buffer(
            self.targets[0].texture().as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &dst,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * SIZE[0]),
                    rows_per_image: Some(SIZE[1]),
                },
            },
            size,
        );
        self.q.submit([e.finish()]);
        self.bytes(&dst)
    }
    fn events(&self) {
        let mut l = self.load(&self.read("events"));
        for (seq, t, v) in [(2, 5, 2.), (1, 5, 1.), (1, 5, 1.), (3, 20, 0.)] {
            l.write_tick("stick", 1, t, &[v, 0.], Some(seq)).unwrap();
        }
        assert!(matches!(l.write_tick("stick", 1, 5, &[9., 0.], Some(4)), Err(Error::Usage(_))));
        assert!(matches!(l.write_tick("stick", 1, 20, &[8., 0.], Some(3)), Err(Error::Usage(_))));
        for (tick, want) in [(4, [0., 0.]), (10, [12., 2.]), (100, [120., 3.]), (10, [12., 2.]), (20, [120., 3.])] {
            l.set_time(tick as f32 / 120.);
            let mut e = self.d.create_command_encoder(&Default::default());
            l.advance_rows(&mut e);
            let buf = self.stage(2 * 672);
            l.copy_state(&mut e, 0, None, &buf, 0).unwrap();
            self.q.submit([e.finish()]);
            let bytes = self.bytes(&buf);
            let read = |k| f32::from_le_bytes(bytes[k..k + 4].try_into().unwrap());
            assert_eq!([read(16), read(20)], [0., 0.]);
            assert_eq!([read(672 + 16), read(672 + 20)], want);
        }
        let mut instance = l.instance(&self.d, &self.q).unwrap();
        instance.write_tick(&l, "stick", 1, 20, &[0., 0.], Some(3)).unwrap();
        assert!(instance.write_tick(&l, "stick", 1, 21, &[0., 0.], Some(3)).is_err());
        instance.write_tick(&l, "stick", 0, 21, &[5., 0.], Some(3)).unwrap();
    }
    fn bench(&self, name: &str) {
        let b = self.read(name);
        let size = match b.resources[b.row_steps[0].seed as usize].shape {
            Shape::Rows { rows, .. } => rows,
            _ => unreachable!(),
        };
        let mut l = self.load(&b);
        l.set_timestamps(Some(self.queries.clone()));
        if name == "one" {
            let mut e = self.d.create_command_encoder(&Default::default());
            l.bake_rows(&mut e, 0).unwrap();
            self.q.submit([e.finish()]);
            let t = self.spans();
            println!("480-tick bake {:.6} ms", t[1] - t[0]);
        }
        for row in 0..size {
            l.write_tick("stick", row, 1, &[0.0, 1.2], None).unwrap();
        }
        let mut motion = vec![];
        let mut whole = vec![];
        // Move through lift and swing on a warm path. The game view is identical for both frame arms.
        if !b.passes.is_empty() {
            l.use_camera("game").unwrap();
        }
        for i in 0..64 {
            let mut e = self.d.create_command_encoder(&Default::default());
            l.set_time((90 + 2 * i) as f32 / 120.0);
            if b.passes.is_empty() {
                l.advance_rows(&mut e);
            } else {
                self.encode(&mut l, &mut e);
            }
            self.q.submit([e.finish()]);
            let t = self.spans();
            if i >= 8 {
                motion.push(t[1] - t[0]);
                if !b.passes.is_empty() {
                    whole.push(t[3].max(t[1]) - t[0].min(t[2]));
                }
            }
        }
        println!("bench {name} motion {:?} whole {:?} ms (p50/p95)", quantiles(motion), quantiles(whole));
    }
    fn frozen(&self) {
        let mut b = self.read("one");
        let mut live = self.load(&b);
        let spec = b.row_steps.pop().unwrap();
        let mut frozen = self.load(&b);
        let poses = fs::read(self.dir.join("one.pose")).unwrap();
        let words: Vec<_> = fs::read(self.dir.join("game.camera"))
            .unwrap()
            .as_chunks::<4>().0.iter()
            .map(|v| f32::from_le_bytes(*v))
            .collect();
        for loft in [&mut live, &mut frozen] {
            loft.set_camera(loft_ir::frame::Camera {
                view_proj: words[0..16].try_into().unwrap(),
                inv_view_proj: words[16..32].try_into().unwrap(),
                eye: words[32..36].try_into().unwrap(),
                forward: words[36..40].try_into().unwrap(),
                sun: [0., 1., 0., 0.],
            });
            loft.set_timestamps(Some(self.queries.clone()));
        }
        for (k, v) in [(1, 0.0), (24, 1.2), (204, -2.1)] {
            live.write_tick("stick", 0, k, &[0.0, v], None).unwrap();
        }
        let mut normal = vec![];
        let mut recorded = vec![];
        let mut differences = vec![];
        for i in 0..64 {
            let k = 90 + 2 * i;
            frozen.write_rows(&self.q, spec.output, &poses[k * 928..(k + 1) * 928]).unwrap();
            let mut pair = [0.0; 2];
            for arm in [i % 2, 1 - i % 2] {
                let loft = if arm == 0 { &mut live } else { &mut frozen };
                loft.set_time(k as f32 / 120.0);
                let mut e = self.d.create_command_encoder(&Default::default());
                self.encode(loft, &mut e);
                self.q.submit([e.finish()]);
                let t = self.spans();
                pair[arm] = if arm == 0 { t[3].max(t[1]) - t[0].min(t[2]) } else { t[3] - t[2] };
            }
            if i >= 8 {
                normal.push(pair[0]);
                recorded.push(pair[1]);
                differences.push(pair[0] - pair[1]);
            }
            if i % 16 == 0 {
                assert_eq!(self.picture(&mut live), self.picture(&mut frozen));
            }
        }
        println!(
            "paired frames, identical RGBA; live {:?}, recorded {:?}, difference {:?} ms (p50/p95)",
            quantiles(normal),
            quantiles(recorded),
            quantiles(differences)
        );
    }
}
fn quantiles(mut values: Vec<f64>) -> Vec<f64> {
    if values.is_empty() {
        return vec![];
    }
    values.sort_by(f64::total_cmp);
    [0.50, 0.95].map(|p| values[((values.len() - 1) as f64 * p).round() as usize]).into()
}
#[test]
#[ignore = "score recorded takes: bun tools/motion_fixtures.ts; set LOFT_TAKE_DIR and run this check"]
fn recorded_take_score() {
    let adapter = pollster::block_on(wgpu::Instance::default().request_adapter(&Default::default())).unwrap();
    let (d, q) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::TIMESTAMP_QUERY,
        ..Default::default()
    }))
    .unwrap();
    let queries =
        d.create_query_set(&wgpu::QuerySetDescriptor { label: None, ty: wgpu::QueryType::Timestamp, count: 4 });
    let targets = [CANVAS.color, CANVAS.depth].map(|format| {
        d.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: SIZE[0], height: SIZE[1], depth_or_array_layers: 1 },
            format,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
        .create_view(&Default::default())
    });
    let dir = std::env::var_os("LOFT_TAKE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../out/motion-score"));
    let r = Run { d, q, dir, targets, queries };
    r.events();
    let base = r.trace("one");
    r.trace("low");
    r.trace("broken");
    r.trace("deck");
    r.trace("pads");
    for name in ["tracking","saturated","flight"] { r.trace(name); }
    let (_, key, rgba) = r.replay(4, false, &base);
    let mut baked=r.load(&r.read("one"));baked.set_time(4.0);baked.use_camera("game").unwrap();
    assert_eq!(key,baked.frame_key(0,SIZE));assert_eq!(rgba,r.picture(&mut baked));
    assert!(rgba.as_chunks::<4>().0.iter().any(|p| p != &rgba[..4]));
    for group in [2, 1, 6] {
        let (_, other, pix) = r.replay(group, false, &base);
        assert_eq!(key, other);
        assert_eq!(rgba, pix);
    }
    let (alt, other, pix) = r.replay(2, true, &base);
    assert_ne!(key, other);
    assert_ne!(rgba, pix);
    fs::write(r.dir.join("alternate.state"), alt).unwrap();
    for name in ["one", "5", "256", "1000"] {
        r.bench(name);
    }
    r.frozen();
}
