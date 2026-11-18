//! Lane 2.1 pass/fail (lean Rust plan §8.2): over the corpus, the Kernel/Resource plan accounts for every TS table,
//! recurrence and compute pass (same rows and lanes, `Steps` keep the same history, per-frame dispatches <= TS's), and
//! Const fills equal TS's f32 fills within 1 ulp. Input: `bun rust/crates/loft-core/dump_kernels.ts >
//! out/lr21/kernels.jsonl`. Run: `cargo test --release --test it kernels -- --nocapture`.

use loft_core::kernel::{Decl, Fill, Job, Source, kept, plan};
use loft_core::layout::layout;
use loft_core::schedule::schedule;
use loft_ir::bundle::{F32, Init, Keep, Shape, TexFormat, When};
use loft_ir::graph::{Decl as GDecl, Graph, Leaf, Node, Op, Prim, Sel, Tag, Ty};
use serde_json::Value;
use std::collections::HashMap;
use std::io::BufRead;

fn ty(v: &Value) -> Ty {
    match v.as_str().unwrap() {
        "f32" => Ty::F32,
        "vec2" => Ty::V2,
        "vec3" => Ty::V3,
        "vec4" => Ty::V4,
        "bool" => Ty::Bool,
        t => panic!("type {t}"),
    }
}
fn u(v: &Value) -> u32 {
    v.as_u64().unwrap() as u32
}
/// "load", "derived"/"frame" (at most per frame), or `{steps: {rate, from, count, keep}}`.
fn when(v: &Value) -> When {
    match (v.as_str(), &v["steps"]) {
        (Some("load"), _) => When::Load,
        (Some(_), _) => When::Frame,
        (None, st) => When::Steps {
            dt: F32((1.0 / st["rate"].as_f64().unwrap() as f32).to_bits()),
            from: F32((st["from"].as_f64().unwrap() as f32).to_bits()),
            count: u(&st["count"]),
            keep: Keep::Every(u(&st["keep"])),
        },
    }
}
fn ids(v: &Value) -> Vec<u32> {
    v.as_array().unwrap().iter().map(u).collect()
}

/// The fixture scene as loft-ir's graph, its sources and kernel decls (opaque TS fills are lane 2.4's: counted, and
/// stood in for by a per-frame curve when their role is per frame, by an unwritten resource when static).
struct Scene {
    g: Graph,
    sources: Vec<Source>,
    decls: Vec<Decl>,
    /// decl -> fixture kernel index
    names: Vec<String>,
    opaque: usize,
}

fn scene(s: &Value) -> Scene {
    let mut g = Graph {
        nodes: Box::new([]),
        args: Box::new([]),
        lits: Box::new([]),
        rows: Box::new([]),
        blobs: Box::new([]),
        syms: "".into(),
        sym_ends: Box::new([]),
        sites: Box::new([]),
        node_sites: Box::new([]),
        decls: Box::new([]),
        roots: Box::new([]),
    };
    let (mut nodes, mut args, mut lits, mut rows, mut decls, mut roots) = (vec![], vec![], vec![], vec![], vec![], vec![]);
    let mut names: HashMap<String, u32> = HashMap::new();
    let mut intern = |n: &str| {
        let k = names.len() as u32;
        *names.entry(n.to_string()).or_insert(k)
    };
    let decl = |tag: Tag, arity: usize, rs: Vec<u32>, decls: &mut Vec<GDecl>, roots: &mut Vec<u32>| {
        decls.push(GDecl { tag, arity: arity as u32, name: None, roots: roots.len() as u32, n: rs.len() as u32 });
        roots.extend(rs);
        decls.len() as u32 - 1
    };
    // Loops first (decl k = loop k); the fixture writes a loop's parts before the node that names it.
    let loops = s["loops"].as_array().unwrap();
    for l in loops {
        let (init, state, next) = (ids(&l["init"]), ids(&l["state"]), ids(&l["next"]));
        let mut rs = vec![u(&l["count"])];
        rs.extend(&init);
        rs.extend(&state);
        rs.push(u(&l["index"]));
        rs.extend(&next);
        if let Some(st) = l["stop"].as_u64() {
            rs.push(st as u32);
        }
        decl(Tag::Loop, init.len(), rs, &mut decls, &mut roots);
    }
    for n in s["nodes"].as_array().unwrap() {
        let t = ty(&n["ty"]);
        let a: Vec<u32> = match n["k"].as_str().unwrap() {
            "prim" | "vec" | "raw" => ids(&n["args"]),
            "get" => vec![u(&n["a"])],
            "fetch" => vec![u(&n["index"])],
            _ => vec![],
        };
        let op = match n["k"].as_str().unwrap() {
            "lit" => {
                lits.extend(n["v"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap().to_bits()));
                Op::Lit((lits.len() - n["v"].as_array().unwrap().len()) as u32)
            }
            "var" => Op::Leaf(Leaf::Bind(intern(n["name"].as_str().unwrap()))),
            "time" => Op::Leaf(Leaf::Time),
            "param" => Op::Leaf(Leaf::Param(intern(n["name"].as_str().unwrap()))),
            "input" => Op::Leaf(Leaf::Input(intern(n["name"].as_str().unwrap()))),
            "prim" => {
                let f = n["fn"].as_str().unwrap();
                Op::Prim(*Prim::ALL.iter().find(|p| p.name() == f).unwrap_or_else(|| panic!("prim {f}")))
            }
            "vec" => Op::Prim(Prim::Vec),
            "get" => {
                let sel = ids(&n["sel"]);
                Op::Swizzle(Sel { len: sel.len() as u8, comps: sel.iter().enumerate().fold(0, |c, (k, &x)| c | (x as u8) << (2 * k)) })
            }
            "fetch" => Op::Fetch { kernel: u(&n["res"]), lane: u(&n["lane"]) as u8 },
            "raw" => Op::Raw(0),
            "loop" => Op::Loop { decl: u(&n["loop"]), k: u(&n["comp"]) as u8 },
            k => panic!("node {k}"),
        };
        nodes.push(Node { op, ty: t, args: args.len() as u32, n: a.len() as u32 });
        args.extend(a);
    }
    let res = s["resources"].as_array().unwrap();
    let sources: Vec<Source> = res
        .iter()
        .map(|r| {
            if let Some(sm) = r["samples"].as_array() {
                rows.extend(sm.iter().map(|x| (x.as_f64().unwrap() as f32).to_bits()));
                Source::Curve { at: (rows.len() - sm.len()) as u32, len: sm.len() as u32 }
            } else if let Some(o) = r["shape"].as_object() {
                let e = ids(&o["texture"]);
                Source::Made(Shape::Texture { extent: [e[0], e[1], *e.get(2).unwrap_or(&1)], format: TexFormat::R32Float })
            } else {
                Source::Made(Shape::Rows { rows: u(&r["rows"]), lanes: u(&r["lanes"]) as u8 })
            }
        })
        .collect();
    let role: HashMap<&str, &str> =
        s["ts"]["tables"].as_array().unwrap().iter().map(|t| (t["name"].as_str().unwrap(), t["role"].as_str().unwrap())).collect();
    let (mut kd, mut kn, mut opaque) = (vec![], vec![], 0);
    for k in s["kernels"].as_array().unwrap() {
        let writes = u(&k["res"]);
        let name = res[writes as usize]["name"].as_str().unwrap().to_string();
        let when = when(&k["when"]);
        let f = &k["fill"];
        let fill = if let Some(m) = f.get("map") {
            let mut rs = vec![u(&m["i"])];
            rs.extend(ids(&m["lanes"]));
            let fd = decl(Tag::Fn, 1, rs, &mut decls, &mut roots);
            Fill::Map { start: u(&m["start"]), count: u(&m["count"]), f: fd }
        } else if let Some(sc) = f.get("scan") {
            let init = decl(Tag::Fn, 0, ids(&sc["init"]), &mut decls, &mut roots);
            let state = ids(&sc["state"]);
            let mut rs = state.clone();
            rs.push(u(&sc["i"]));
            rs.extend(ids(&sc["next"]));
            rs.extend(ids(&sc["lanes"]));
            let step = decl(Tag::Fn, state.len() + 1, rs, &mut decls, &mut roots);
            Fill::Scan { first: u(&sc["first"]), steps: u(&sc["steps"]), init, step }
        } else if let Some(r) = f.get("raw") {
            let d = ids(&r["dispatch"]);
            Fill::Raw { module: 0, dispatch: [d[0], *d.get(1).unwrap_or(&1), *d.get(2).unwrap_or(&1)], reads: ids(&r["reads"]).into() }
        } else {
            // A per-frame opaque fill stands in as a Map of `time` (per frame, the TS rows and lanes); a static one
            // as an unwritten resource.
            opaque += 1;
            if role.get(name.as_str()) == Some(&"static") {
                continue;
            }
            let (r, lanes) = (u(&res[writes as usize]["rows"]), u(&res[writes as usize]["lanes"]));
            let at = nodes.len() as u32;
            nodes.push(Node { op: Op::Leaf(Leaf::Bind(intern(&format!("_opaque_{name}")))), ty: Ty::F32, args: 0, n: 0 });
            nodes.push(Node { op: Op::Leaf(Leaf::Time), ty: Ty::F32, args: 0, n: 0 });
            nodes.push(Node { op: Op::Prim(Prim::Vec), ty: Ty::V4, args: args.len() as u32, n: 1 });
            args.push(at + 1);
            let mut rs = vec![at];
            rs.extend(std::iter::repeat_n(at + 2, lanes as usize));
            Fill::Map { start: 0, count: r, f: decl(Tag::Fn, 1, rs, &mut decls, &mut roots) }
        };
        kd.push(Decl { writes, fill, when });
        kn.push(name);
    }
    g.nodes = nodes.into();
    g.args = args.into();
    g.lits = lits.into();
    g.rows = rows.into();
    g.decls = decls.into();
    g.roots = roots.into();
    Scene { g, sources, decls: kd, names: kn, opaque }
}

/// Distance in f32 ulps (ordered bit patterns); equal NaNs are 0.
fn ulps(a: u32, b: u32) -> u64 {
    let ord = |x: u32| if x & 0x8000_0000 != 0 { -((x & 0x7fff_ffff) as i64) } else { x as i64 };
    if f32::from_bits(a).is_nan() && f32::from_bits(b).is_nan() { 0 } else { (ord(a) - ord(b)).unsigned_abs() }
}

/// What the corpus run measured: structural misses, and Const fills further than 1 ulp from TS.
struct Report {
    fails: Vec<String>,
    worst: Vec<(u64, String)>,
}

fn report() -> &'static Report {
    static R: std::sync::OnceLock<Report> = std::sync::OnceLock::new();
    R.get_or_init(measure)
}

#[test]
fn kernels_account_for_the_ts_plan() {
    let r = report();
    assert!(r.fails.is_empty(), "{} misses: {:#?}", r.fails.len(), &r.fails[..r.fails.len().min(40)]);
}

/// Plan §3.4 moves fills to an f32 tape; TS filled in f64 and rounded once. A body that cancels (a transported
/// frame's cross products, cos(pi/2)) differs by more than 1 ulp of a near-zero result. Measured 19 Nov: 79 of 160
/// fills, worst |d| / max(1, |want|) = 9.2e-6. The gate's 1 ulp is the owner's to re-score; this test states it.
#[test]
fn const_fills_within_1_ulp_of_ts() {
    let r = report();
    assert!(r.worst.is_empty(), "{} fills over 1 ulp; worst {:?}", r.worst.len(), &r.worst[..r.worst.len().min(8)]);
}

fn measure() -> Report {
    let path = std::env::var("LR21_FIXTURE")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../../out/lr21/kernels.jsonl").into());
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|e| panic!("{path}: {e}. Write it first: bun rust/crates/loft-core/dump_kernels.ts > out/lr21/kernels.jsonl"));
    let (mut scenes, mut fails) = (0, Vec::<String>::new());
    let mut tot: HashMap<&str, u64> = HashMap::new();
    let mut worst: Vec<(u64, String)> = vec![];
    let mut worst_rel = 0f64;
    for line in std::io::BufReader::new(file).lines() {
        let s: Value = serde_json::from_str(&line.unwrap()).unwrap();
        if s["outcome"] != "built" {
            continue;
        }
        scenes += 1;
        let name = s["scene"].as_str().unwrap().to_string();
        let sc = scene(&s);
        let t0 = std::time::Instant::now();
        let p = match plan(&sc.g, &sc.sources, &sc.decls) {
            Ok(p) => p,
            Err(e) => {
                fails.push(format!("{name}: plan refused: {e:?}"));
                continue;
            }
        };
        let stages = schedule(&p, &sc.decls).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let lay = layout(&p);
        *tot.entry("plan_us").or_default() += t0.elapsed().as_micros() as u64;
        if let Err(e) = &lay {
            fails.push(format!("{name}: layout refused: {e:?}"));
        }
        *tot.entry("opaque").or_default() += sc.opaque as u64;

        // (1) every TS table, recurrence and compute output has a resource of the same rows and lanes.
        let res = s["resources"].as_array().unwrap();
        let by_name: HashMap<&str, usize> = res.iter().enumerate().map(|(k, r)| (r["name"].as_str().unwrap(), k)).collect();
        let recur: HashMap<&str, &Value> =
            s["ts"]["recurrences"].as_array().unwrap().iter().map(|r| (r["name"].as_str().unwrap(), r)).collect();
        for t in s["ts"]["tables"].as_array().unwrap() {
            let (tn, kind) = (t["name"].as_str().unwrap(), t["kind"].as_str().unwrap());
            if t["frameValues"].as_bool().unwrap() {
                *tot.entry("ts_frame_values").or_default() += u64::from(u(&t["lanes"]));
                continue;
            }
            if kind == "replay" {
                *tot.entry("ts_replay_tables").or_default() += 1;
                continue;
            }
            let (want_rows, want_lanes) = match kind {
                "kept" => {
                    let r = recur.values().find(|r| tn.starts_with(r["name"].as_str().unwrap())).unwrap();
                    (u(&r["entries"]), u(&r["lanes"]))
                }
                _ => (u(&t["entries"]), u(&t["lanes"])),
            };
            match by_name.get(tn).map(|&r| &p.resources[r].shape) {
                Some(Shape::Rows { rows, lanes }) if (*rows, u32::from(*lanes)) == (want_rows, want_lanes) => {
                    *tot.entry("tables_ok").or_default() += 1
                }
                got => fails.push(format!("{name}: table {tn} ({kind}) {want_rows}x{want_lanes}, plan {got:?}")),
            }
        }
        *tot.entry("compute_passes").or_default() += s["ts"]["compute"].as_array().unwrap().len() as u64;

        // (2) Steps keep the same history.
        for (k, d) in s["kernels"].as_array().unwrap().iter().filter(|k| k["when"].is_object()).enumerate() {
            let (st, w) = (&d["when"]["steps"], when(&d["when"]));
            let (got, want) = (kept(&w), u(&st["states"]));
            *tot.entry("steps").or_default() += 1;
            if got != want {
                fails.push(format!("{name}: steps kernel {k} keeps {got} states; TS {want}"));
            }
        }

        // (3) per-frame dispatches <= TS's (pose stages, replays included, plus per-frame compute passes).
        let frame_stages = stages.iter().filter(|st| matches!(p.kernels[st[0] as usize].when, When::Frame)).count() as u64;
        let ts_frame = s["ts"]["stages"].as_array().unwrap().len() as u64
            + s["ts"]["compute"].as_array().unwrap().iter().filter(|c| c["when"] == "frame").count() as u64;
        *tot.entry("rust_frame_dispatches").or_default() += frame_stages;
        *tot.entry("ts_frame_dispatches").or_default() += ts_frame;
        if frame_stages > ts_frame {
            fails.push(format!("{name}: {frame_stages} dispatches per frame; TS {ts_frame}"));
        }

        // (4) Const fills equal TS's f32 fills within 1 ulp, on the rows they write.
        let on_gpu: Vec<bool> = {
            let mut v = vec![false; sc.decls.len()];
            for k in &p.kernels {
                if let Job::Decl(d) = k.job {
                    v[d as usize] = true;
                }
            }
            v
        };
        for (k, d) in sc.decls.iter().enumerate() {
            let class = match (&d.when, on_gpu[k]) {
                (_, false) => "compile",
                (When::Load, true) => "load",
                (When::Frame, true) => "frame",
                (When::Steps { .. }, true) => "steps",
            };
            *tot.entry(class).or_default() += 1;
            if on_gpu[k] {
                continue;
            }
            let (Init::Bytes(sec), Some(want)) = (&p.resources[d.writes as usize].init, s["fills"].get(&sc.names[k])) else {
                *tot.entry("compile_no_ts_fill").or_default() += 1;
                println!("compiled without a TS fill to compare: {name} {}", sc.names[k]);
                continue;
            };
            let lanes = match p.resources[d.writes as usize].shape {
                Shape::Rows { lanes, .. } => lanes as usize * 4,
                Shape::Texture { .. } | Shape::ViewSized { .. } | Shape::Frame => unreachable!(),
            };
            let rows = match d.fill {
                Fill::Map { start, count, .. } => start..start + count,
                Fill::Scan { first, steps, .. } => first..first + steps,
                Fill::Raw { .. } => unreachable!(),
            };
            let bytes = &p.sections[*sec as usize];
            let mut far = 0;
            for r in rows {
                for c in 0..lanes {
                    let at = r as usize * lanes + c;
                    let got = u32::from_le_bytes(bytes[4 * at..4 * at + 4].try_into().unwrap());
                    let w = u(&want[at]);
                    let du = ulps(got, w);
                    let (gf, wf) = (f32::from_bits(got) as f64, f32::from_bits(w) as f64);
                    let rel = if du == 0 { 0.0 } else { (gf - wf).abs() / wf.abs().max(1.0) };
                    worst_rel = worst_rel.max(rel);
                    *tot.entry("fill_values").or_default() += 1;
                    if du > 1 {
                        *tot.entry("fill_values_over_1ulp").or_default() += 1;
                    }
                    far = far.max(du);
                }
            }
            *tot.entry("compile_compared").or_default() += 1;
            if far > 1 {
                worst.push((far, format!("{name}: {} rows {:?}", sc.names[k], match d.fill {
                    Fill::Map { start, count, .. } => (start, count),
                    Fill::Scan { first, steps, .. } => (first, steps),
                    Fill::Raw { .. } => (0, 0),
                })));
            }
        }
        *tot.entry("lifted").or_default() += p.lifted.len() as u64;
        *tot.entry("kernels").or_default() += sc.decls.len() as u64;
    }
    worst.sort_unstable_by(|a, b| b.cmp(a));
    let mut t: Vec<_> = tot.into_iter().collect();
    t.sort();
    println!("scenes {scenes}\n{t:?}\nworst |got - want| / max(1, |want|): {worst_rel:e}\nfills over 1 ulp: {}\n{:?}", worst.len(), &worst[..worst.len().min(8)]);
    Report { fails, worst }
}
