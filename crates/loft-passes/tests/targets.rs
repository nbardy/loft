use loft_ir::{
    BindAccess, Bundle, DispatchExtent, ElemFormat, ImageFormat, PassRun,
    execution::{BlendMode, CullMode, DESCRIPTOR, Execution, RasterState},
};
use loft_passes::{bundle, source::Graph};
use std::{fs, path::Path};

const TRIANGLE: &str = r"
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let p = array(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
    return vec4f(p[i], 0.5, 1.0);
}";

fn lower(text: &str, shaders: &[(&str, &str)]) -> (loft_passes::program::Program, Bundle) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = tempfile::tempdir().unwrap();
    for &(name, source) in shaders {
        fs::write(dir.path().join(name), source).unwrap();
    }
    let src = Graph::text(&root, &dir.path().join("main.loft"), text);
    let (program, _, compiled) =
        loft_passes::compile(&src, &[]).unwrap_or_else(|d| panic!("{}", d.render(&src)));
    let bundle = bundle::lower(&program, &compiled, (61, 39), |_, _| vec![0; 256]).unwrap();
    (program, bundle)
}

#[test]
fn authored_draws_share_the_gbuffer_across_passes() {
    let wgsl = format!("{TRIANGLE}{}", r"
@group(1) @binding(0) var GBuffer: texture_2d<f32>;
@fragment fn geometry() -> @location(0) vec4f {
    return vec4f(0.25, 0.5, 0.75, 1.0);
}
@fragment fn display(@builtin(position) p: vec4f) -> @location(0) vec4f {
    return textureLoad(GBuffer, vec2i(p.xy), 0);
}");
    let (program, bundle) = lower(r#"
target GBuffer: rgba16f view
target Depth: depth32f view
schedule Main {
    pass geometry writes [GBuffer, Depth] {
        draw("frame.wgsl", "vs", "geometry", cull: back)
    }
    pass display reads [GBuffer] writes [Swapchain] {
        draw("frame.wgsl", "vs", "display", blend: alpha)
    }
}
"#, &[("frame.wgsl", &wgsl)]);
    let [first, second] = bundle.passes.as_slice() else { panic!("expected two authored draws") };
    assert!(first.wave < second.wave);
    assert!(!program.passes[0].presents());
    assert!(program.passes[1].presents());
    let PassRun::RenderBundle { color_targets, depth: Some(depth), msaa: 1, .. } = &first.run
        else { panic!("expected a color and depth draw") };
    let [gbuffer] = color_targets.as_ref() else { panic!("expected one color target") };
    let texture = |id| bundle.resources.textures.iter().find(|t| t.table == id).unwrap();
    assert!(matches!(texture(gbuffer.table).format, ImageFormat::Rgba16Float));
    assert!(matches!(texture(depth.table).format, ImageFormat::Depth32Float));
    assert_eq!(texture(gbuffer.table).extent, [61, 39, 1]);
    assert_eq!(texture(depth.table).extent, [61, 39, 1]);
    assert!(first.bindings.is_empty());
    let [sample] = second.bindings.as_ref() else { panic!("expected one sampled binding") };
    assert_eq!(sample.resource, gbuffer.table);
    assert_eq!(sample.access, BindAccess::SampledTex);
    let PassRun::RenderBundle { color_targets, depth: None, .. } = &second.run
        else { panic!("expected a swapchain draw") };
    assert_eq!(color_targets.len(), 1);
    assert!(!bundle.resources.textures.iter().any(|t| t.table == color_targets[0].table));
    let execution = Execution::read(bundle.tables.iter().find(|t| t.id == DESCRIPTOR).unwrap()).unwrap();
    assert_eq!(execution.raster, [
        RasterState { pass: first.id, cull: CullMode::Back, blend: BlendMode::Opaque },
        RasterState { pass: second.id, cull: CullMode::None, blend: BlendMode::Alpha },
    ]);
}

#[test]
fn compute_and_draw_share_a_table_and_scaled_target() {
    let seed = r"
struct Cell { mass: f32, }
@group(2) @binding(0) var<storage, read_write> Cells: array<Cell>;
@compute @workgroup_size(1) fn seed() { Cells[0].mass = 0.25; }";
    let solve = r"
struct Cell { mass: f32, }
@group(1) @binding(0) var<storage, read> Cells: array<Cell>;
@group(1) @binding(1) var Flow: texture_storage_2d<rgba16float, write>;
@compute @workgroup_size(8, 4) fn solve(@builtin(global_invocation_id) p: vec3u) {
    if all(p.xy < textureDimensions(Flow)) {
        textureStore(Flow, vec2i(p.xy), vec4f(Cells[0].mass));
    }
}";
    let display = format!("{TRIANGLE}{}", r"
struct Cell { mass: f32, }
@group(1) @binding(0) var<storage, read> Cells: array<Cell>;
@group(1) @binding(1) var Flow: texture_2d<f32>;
@fragment fn display() -> @location(0) vec4f {
    return textureLoad(Flow, vec2i(0), 0) * Cells[0].mass;
}");
    let (_, bundle) = lower(r#"
struct Cell { mass: f }
target Flow: rgba16float view * 0.5 msaa 1
schedule Main {
    table(Cells, row: Cell, capacity: 1)
    pass seed writes [Cells] { dispatch("seed.wgsl", "seed", 1) }
    pass solve reads [Cells] writes [Flow] { dispatch("solve.wgsl", "solve", Flow) }
    pass display reads [Flow, Cells] writes [Swapchain] { draw("display.wgsl", "vs", "display") }
}
"#, &[("seed.wgsl", seed), ("solve.wgsl", solve), ("display.wgsl", &display)]);
    let [seed, solve, display] = bundle.passes.as_slice() else { panic!("expected seed, solve and draw") };
    assert!(seed.wave < solve.wave && solve.wave < display.wave);
    assert!(matches!(solve.run, PassRun::Compute { dispatch: DispatchExtent::Fixed([4, 5, 1]), .. }));
    let table = seed.bindings[0].resource;
    assert!(matches!(bundle.tables[table.0 as usize].format, ElemFormat::Struct(_)));
    let [cells, flow] = solve.bindings.as_ref() else { panic!("expected a buffer and storage texture") };
    assert_eq!((cells.resource, cells.access), (table, BindAccess::ReadStorage));
    assert_eq!(flow.access, BindAccess::StorageTex);
    assert_eq!(bundle.resources.textures.iter().find(|t| t.table == flow.resource).unwrap().extent, [31, 20, 1]);
    let [cells, sampled] = display.bindings.as_ref() else { panic!("expected a buffer and sampled texture") };
    assert_eq!(cells.resource, table);
    assert_eq!((sampled.resource, sampled.access), (flow.resource, BindAccess::SampledTex));
}

#[test]
fn draw_keeps_declared_color_order_and_multisampling() {
    let wgsl = format!("{TRIANGLE}{}", r"
struct Fragment {
    @location(0) color: vec4f,
    @location(1) vector: vec4f,
}
@fragment fn geometry() -> Fragment {
    return Fragment(vec4f(1.0), vec4f(0.0));
}");
    let (_, bundle) = lower(r#"
target ZColor: rgba16f 16 x 8 msaa 4
target AVector: rgba32f 16x8 msaa 4
target Depth: depth32float 16 x 8 msaa 4
schedule Main {
    pass geometry writes [ZColor, Depth, AVector] { draw("frame.wgsl", "vs", "geometry") }
}
"#, &[("frame.wgsl", &wgsl)]);
    let [pass] = bundle.passes.as_slice() else { panic!("expected one authored draw") };
    let PassRun::RenderBundle { color_targets, depth: Some(depth), msaa: 4, .. } = &pass.run
        else { panic!("expected a multisampled draw") };
    let [color, vector] = color_targets.as_ref() else { panic!("expected two color targets") };
    let texture = |id| bundle.resources.textures.iter().find(|t| t.table == id).unwrap();
    assert!(matches!(texture(color.table).format, ImageFormat::Rgba16Float));
    assert!(matches!(texture(vector.table).format, ImageFormat::Rgba32Float));
    for id in [color.table, vector.table, depth.table] {
        assert_eq!(texture(id).extent, [16, 8, 1]);
        assert_eq!(texture(id).samples, 4);
    }
}
