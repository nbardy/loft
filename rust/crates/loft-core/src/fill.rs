//! Fill kernels on the GPU (plan §1.4): each Map or Scan kernel the plan leaves becomes one module, its bodies as graph
//! functions (emit, lane 2.2) and a compute entry `loft_fill` over the host's binding contract (loft-gpu docs): the
//! table it writes at `@group(2) @binding(0)`, what its bodies read at `@group(1)`. One module per kernel, so a
//! kernel never binds the table it writes as read-only beside it.
//!
//! - Map: `@workgroup_size(64)`, one invocation per row of the run; row `start + i` gets each lane at `i`.
//! - Scan: one invocation; the state starts at `init`, step j writes row `first + j`'s lanes from the state before
//!   the step, then advances it by `next` (src/expr.ts `ScanStep`).
use crate::emit::{self, Slots, ty_name};
use crate::kernel;
use loft_ir::Error;
use loft_ir::error::SceneFault;
use loft_ir::graph::{Graph, GraphFn};
use std::fmt::Write;

/// One kernel's module: WGSL (without the `lf` alias) and the resources it reads in binding order.
pub struct Module {
    pub text: String,
    pub reads: Vec<u32>,
    /// Rows it covers per dispatch (`Body::Map.rows`, `Body::Scan.rows`).
    pub rows: u32,
}

/// The module of decl `d`'s fill. `g` gains the graph functions it exports (as the driver's roots do).
pub fn module(g: &mut Graph, at: &Slots, d: &kernel::Decl) -> Result<Module, Error> {
    let [_, lanes] = at.kernels[d.writes as usize];
    let first = g.fns.len() as u32;
    let add = |binders: &[u32], body: u32, g: &mut Graph| {
        let ret = g.nodes[body as usize].ty;
        g.fns = g.fns.iter().cloned().chain([GraphFn { name: 0, ret, binders: binders.into(), body }]).collect();
        g.fns.len() as u32 - 1
    };
    let mut names: Vec<String> = Vec::new();
    let mut fns: Vec<u32> = Vec::new();
    let mut glue = String::from("@group(2) @binding(0) var<storage, read_write> loft_out: array<vec4<f32>>;\n");
    let rows = match &d.fill {
        kernel::Fill::Map { start, count, index, lanes: ls } => {
            for (l, &x) in ls.iter().enumerate() {
                fns.push(add(&[*index], x, g));
                names.push(format!("loft_lane{l}"));
            }
            let _ = writeln!(
                glue,
                "@compute @workgroup_size(64) fn loft_fill(@builtin(global_invocation_id) id: vec3<u32>) {{\n  \
                 if (id.x >= {count}u) {{ return; }}\n  let i = lf(f32(id.x));\n  let row = ({start}u + id.x) * {lanes}u;"
            );
            for l in 0..ls.len() {
                let _ = writeln!(glue, "  loft_out[row + {l}u] = vec4<f32>(loft_lane{l}(i));");
            }
            *count
        }
        kernel::Fill::Scan { first, steps, init, state, index, next, lanes: ls } => {
            let mut params: Vec<u32> = state.to_vec();
            params.push(*index);
            for (j, &x) in init.iter().enumerate() {
                fns.push(add(&[], x, g));
                names.push(format!("loft_init{j}"));
            }
            for (j, &x) in next.iter().enumerate() {
                fns.push(add(&params, x, g));
                names.push(format!("loft_next{j}"));
            }
            for (l, &x) in ls.iter().enumerate() {
                fns.push(add(&params, x, g));
                names.push(format!("loft_lane{l}"));
            }
            let args: String = (0..state.len()).map(|j| format!("s{j}, ")).collect::<String>() + "i";
            let _ = writeln!(glue, "@compute @workgroup_size(1) fn loft_fill() {{");
            for j in 0..state.len() {
                let ty = ty_name(g.nodes[state[j] as usize].ty, "lf");
                let _ = writeln!(glue, "  var s{j}: {ty} = loft_init{j}();");
            }
            let _ = writeln!(
                glue,
                "  for (var j = 0u; j < {steps}u; j++) {{\n    let i = lf(f32(j));\n    let row = ({first}u + j) * {lanes}u;"
            );
            for l in 0..ls.len() {
                let _ = writeln!(glue, "    loft_out[row + {l}u] = vec4<f32>(loft_lane{l}({args}));");
            }
            for j in 0..state.len() {
                let _ = writeln!(glue, "    let n{j} = loft_next{j}({args});");
            }
            for j in 0..state.len() {
                let _ = writeln!(glue, "    s{j} = n{j};");
            }
            let _ = writeln!(glue, "  }}");
            1
        }
        kernel::Fill::Raw { .. } => unreachable!("the wire carries no raw kernel yet; the driver plans none"),
    };
    glue.push_str("}\n");
    debug_assert_eq!(first as usize + fns.len(), g.fns.len());
    let exports: Vec<(&str, u32)> = names.iter().map(String::as_str).zip(fns.iter().copied()).collect();
    let m = emit::module(g, &exports, at)?;
    let text = format!("{}\n{glue}", m.text);
    // The graph part was validated by emit; this checks the entry around it, which only a template bug can fail.
    let src = format!("alias lf = f32;\n{text}");
    let refuse = |msg: String| Error::Scene { site: None, fault: SceneFault::Wgsl { line: 0, msg: msg.into() } };
    let parsed = naga::front::wgsl::parse_str(&src).map_err(|e| refuse(e.emit_to_string(&src)))?;
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::default())
        .validate(&parsed)
        .map_err(|e| refuse(e.emit_to_string(&src)))?;
    Ok(Module { text, reads: m.reads, rows })
}
