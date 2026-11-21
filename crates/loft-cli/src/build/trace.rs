//! Replay the Phase 1.5 row-local schedule with its f32 expression twin.
use super::{live_names, write};
use crate::{budget::Failure, flag, gates, sources};
use loft_cli::native::{self, Loaded};
use loft_ir::{ParamMeta, Sources, Span, Verdict};
use loft_parse::ast::{Arg, Expr, ExprKind, Item, NumSuffix, Stmt, Ty, TyExpr};
use loft_passes::{
    Compiled, Prog,
    eval::{Eval, RowVal, Val, concrete},
    program::{MotionSample, Program},
};
use loft_runner::replay::{Input, LoftLog, LoggedInput, identity};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

const DT: f32 = 1.0 / 120.0;
const TWIN: &str = "loft-expression-twin/f32/v1";
type Result<T> = std::result::Result<T, Failure>;

#[derive(Serialize, Deserialize)]
#[serde(remote = "LoftLog", deny_unknown_fields)]
struct JsonLog {
    format_version: u32,
    bundle_hash: [u8; 32],
    device_signature: String,
    dt_s: f32,
    initial_seed: u64,
    params: Vec<(String, [f32; 4])>,
    checkpoints_every: u32,
    #[serde(with = "json_inputs")]
    inputs: Vec<LoggedInput>,
    expected_final_identity: Option<[u8; 32]>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "LoggedInput", deny_unknown_fields)]
struct JsonInput {
    tick: u32,
    seq: u32,
    #[serde(with = "JsonValue")]
    input: Input,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "Input", deny_unknown_fields)]
enum JsonValue {
    Level { name: String, value: [f32; 4] },
    Event { name: String, payload: [f32; 4] },
    PosePacket { name: String, bytes: Vec<u8> },
    Param { name: String, value: [f32; 4] },
    End,
}
mod json_inputs {
    use super::*;
    #[derive(Serialize)]
    struct Borrowed<'a>(#[serde(with = "JsonInput")] &'a LoggedInput);
    #[derive(Deserialize)]
    struct Owned(#[serde(with = "JsonInput")] LoggedInput);

    pub(super) fn serialize<S: serde::Serializer>(
        values: &[LoggedInput],
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        s.collect_seq(values.iter().map(Borrowed))
    }
    pub(super) fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<Vec<LoggedInput>, D::Error> {
        Ok(Vec::<Owned>::deserialize(d)?
            .into_iter()
            .map(|v| v.0)
            .collect())
    }
}

#[derive(Serialize, Deserialize)]
struct Json(#[serde(with = "JsonLog")] LoftLog);

fn usage(e: impl std::fmt::Display) -> Failure {
    Failure::Usage(e.to_string())
}
fn device(e: impl std::fmt::Display) -> Failure {
    Failure::Device(e.to_string())
}

fn read(path: &Path) -> Result<LoftLog> {
    let bytes = fs::read(path).map_err(usage)?;
    if bytes.starts_with(b"LOFTLOG1") {
        LoftLog::decode(&bytes).map_err(usage)
    } else {
        let Json(log) = serde_json::from_slice(&bytes).map_err(usage)?;
        log.validate().map_err(usage)?;
        Ok(log)
    }
}

fn bundle_hash(program: &Program, code: &Compiled) -> Result<[u8; 32]> {
    let bundle = super::artifact::lower(program, code).map_err(Failure::Compile)?;
    Ok(identity(&loft_runner::bundle::encode(&bundle)))
}

fn initial_log(hash: [u8; 32], prog: &Prog<'_>, program: &Program) -> LoftLog {
    LoftLog {
        format_version: 1,
        bundle_hash: hash,
        device_signature: TWIN.into(),
        dt_s: DT,
        initial_seed: 0,
        checkpoints_every: 0,
        expected_final_identity: None,
        inputs: vec![],
        params: live_names(prog)
            .zip(program.params.as_chunks::<4>().0.iter().skip(3).copied())
            .map(|(name, value)| (name.to_owned(), value))
            .collect(),
    }
}

pub(crate) fn requested(args: &[String]) -> bool {
    args.iter()
        .any(|a| matches!(a.as_str(), "--record" | "--replay" | "--format"))
}

pub(crate) fn run(args: &[String], set: &[(String, f64)], render: bool) -> Result<()> {
    let mut args = args.to_vec();
    let recording = flag(&mut args, "--record")?;
    let replay = flag(&mut args, "--replay")?;
    if recording.is_some() == replay.is_some() {
        return Err(usage("supply one of --record or --replay"));
    }
    let json = match (flag(&mut args, "--format")?.as_deref(), &recording) {
        (None, _) | (Some("binary"), Some(_)) => false,
        (Some("json"), Some(_)) => true,
        _ => return Err(usage("--format json|binary requires --record")),
    };
    let time = flag(&mut args, if render { "--time" } else { "--t" })?;
    let out = flag(&mut args, "--out")?;
    let size = flag(&mut args, "--size")?.map_or(Ok((1280, 800)), |s| {
        let (w, h) = s
            .split_once('x')
            .ok_or_else(|| usage("--size requires WxH"))?;
        let dimension = |v: &str| {
            v.parse::<u32>()
                .ok()
                .filter(|&n| n > 0)
                .ok_or_else(|| usage("--size requires positive dimensions"))
        };
        Ok((dimension(w)?, dimension(h)?))
    })?;
    let mut updates = Vec::new();
    while let Some(value) = flag(&mut args, "--live")? {
        let (name, value) = value
            .split_once('=')
            .ok_or_else(|| usage("--live requires NAME=VALUE"))?;
        let value = value
            .parse::<f32>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or_else(|| usage("--live requires a finite scalar"))?;
        updates.push(Input::Param {
            name: name.to_owned(),
            value: [value, 0., 0., 0.],
        });
    }

    let [scene] = args.as_slice() else {
        return Err(usage("expected one scene path"));
    };
    if scene.starts_with('-')
        || (!render && out.is_some())
        || render && out.is_none() && replay.is_some()
        || replay.is_some() && (time.is_some() || !updates.is_empty())
    {
        return Err(usage(
            "replay uses its recorded time and inputs; render replay requires --out",
        ));
    }
    let out = out.map(std::path::PathBuf::from);
    if let Some(out) = &out
        && (out.extension().is_none_or(|s| s != "png")
            || recording.as_ref().is_some_and(|s| Path::new(s) == out))
    {
        return Err(usage("trace rendering requires a distinct .png output"));
    }
    let src = sources(Path::new(scene)).map_err(usage)?;
    let (mut program, _, mut compiled) =
        loft_passes::compile(&src, set).map_err(|d| Failure::Compile(d.render(&src)))?;
    if program.dispatches.iter().any(|dispatch| !dispatch.tier.is_det())
        || compiled.compute.tiers.values().any(|tier| !tier.is_det())
    {
        return Err(Failure::Compile("E504: exact replay requires Det producers".into()));
    }
    let hash = bundle_hash(&program, &compiled)?;
    let mut log = if let Some(path) = replay {
        read(Path::new(&path))?
    } else {
        let mut log = initial_log(hash, &compiled.prog(), &program);
        let seconds = time
            .as_deref()
            .unwrap_or("0")
            .parse::<f64>()
            .map_err(usage)?;
        let ticks = (seconds * 120.).round();
        if !seconds.is_finite() || seconds < 0. || ticks >= f64::from(u32::MAX) {
            return Err(usage("recording time must fit in 120 Hz ticks"));
        }
        for i in updates {
            log.record(0, i).map_err(usage)?;
        }
        log.record(ticks as u32, Input::End).map_err(usage)?;
        log
    };
    // Historical twin logs identify their own backend. New recordings hash the native law outputs.
    let gpu = if recording.is_some() || log.device_signature != TWIN || out.is_some() {
        Some(native::gpu().map_err(device)?)
    } else {
        None
    };
    if recording.is_some() {
        log.device_signature = gpu
            .as_ref()
            .expect("recording device")
            .device_signature
            .clone();
    }
    let result = if log.device_signature == TWIN {
        adapt_checks(&mut compiled)?;
        execute(&compiled, &program, &src, &log, hash)?
    } else {
        execute_native(
            gpu.as_ref().expect("replay device"),
            &compiled,
            &mut program,
            &log,
            hash,
            !render,
        )?
    };
    log.verify_identity(&result.bytes)
        .map_err(|e| Failure::Parity(e.to_string()))?;
    if let Some(out) = out {
        let time = log.validate().map_err(usage)? as f32 * DT;
        let (mut snapshot, _, resolved) = loft_passes::compile_snapshot(&src, set, &result.samples)
            .map_err(|d| Failure::Compile(d.render(&src)))?;
        for (i, name) in live_names(&resolved.prog()).enumerate() {
            snapshot.params[(3 + i) * 4] = result.params[name];
        }
        let gpu = gpu.as_ref().expect("output device");
        let loaded = Loaded::new(gpu, &snapshot, size, None).map_err(device)?;
        loaded.frame(gpu, time);
        gates::write_png(&out, size, &loaded.pixels(gpu).map_err(device)?).map_err(usage)?;
    }
    if let Some(path) = recording {
        log.expected_final_identity = Some(identity(&result.bytes));
        let data = if json {
            serde_json::to_vec_pretty(&Json(log.clone())).map_err(usage)?
        } else {
            log.encode().map_err(usage)?
        };
        write(Path::new(&path), &data)?;
    }
    println!(
        "state {} ({} bytes, {})",
        blake3::hash(&result.bytes),
        result.bytes.len(),
        log.device_signature
    );
    if let Some(verdict) = result.missed
        && !render
    {
        return Err(Failure::Truth(verdict));
    }
    Ok(())
}

struct Outcome {
    bytes: Vec<u8>,
    samples: Vec<MotionSample>,
    params: BTreeMap<String, f32>,
    missed: Option<String>,
}

fn execute(
    compiled: &Compiled,
    program: &Program,
    src: &Sources,
    log: &LoftLog,
    hash: [u8; 32],
) -> Result<Outcome> {
    if log.dt_s.to_bits() != DT.to_bits() || log.initial_seed != 0 {
        return Err(usage("this scene backend uses a 120 Hz step and seed 0"));
    }
    if log.device_signature != TWIN {
        return Err(device(
            "this trace has no tolerance reference for the expression twin",
        ));
    }
    let prog = compiled.prog();
    let mut params: BTreeMap<_, _> = initial_log(hash, &prog, program)
        .params
        .into_iter()
        .map(|(name, value)| (name, value[0]))
        .collect();
    if log.params.len() != params.len() {
        return Err(usage(
            "trace parameter snapshot differs from the scene schema",
        ));
    }
    let schema = super::artifact::parameters(program, compiled).map_err(Failure::Compile)?;
    for (name, value) in &log.params {
        update(&schema, &mut params, name, *value)?;
    }
    let ticks = log.ticks(hash).map_err(usage)?;
    let mut tables = compiled.tables.clone();
    let mut samples = Vec::new();
    let mut missed = None;
    for (tick, events) in ticks {
        apply_inputs(&schema, &mut params, events)?;
        let overrides: Vec<_> = params
            .iter()
            .map(|(name, value)| (name.clone(), f64::from(*value)))
            .collect();
        let mut ev = Eval::new(&prog, &compiled.types, tables, &overrides);
        ev.luts = &compiled.luts;
        ev.time = tick as f32 * DT;
        if tick != 0 {
            step(&mut ev, src)?;
        }
        samples = snapshots(&mut ev, program, src)?;
        for (name, verdict) in gates::check_records(program, &samples) {
            if !matches!(verdict, Verdict::Held { .. }) && missed.is_none() {
                // Preserve failed traces for reproduction. Only a Truth check
                // turns an authored verdict into the command's exit status.
                missed = Some(format!("tick {tick} {name}: {verdict}"));
            }
        }
        tables = ev.tables;
    }

    Ok(Outcome {
        bytes: state_bytes(tables),
        samples,
        params,
        missed,
    })
}

fn state_bytes(tables: loft_passes::eval::Tables) -> Vec<u8> {
    let mut bytes = Vec::new();
    for rows in tables {
        for row in rows {
            for field in row {
                let word = |x: f32| x.to_le_bytes();
                match field {
                    RowVal::F(x) => bytes.extend(word(x)),
                    RowVal::V(x) => bytes.extend(x.into_iter().flat_map(word)),
                    RowVal::U(x) => bytes.extend(x.to_le_bytes()),
                    RowVal::I(x) => bytes.extend(x.to_le_bytes()),
                    RowVal::B(x) => bytes.extend(u32::from(x).to_le_bytes()),
                    RowVal::X(x) => {
                        bytes.extend(x.r.into_iter().flatten().chain(x.t).flat_map(word))
                    }
                }
            }
        }
    }
    bytes
}

fn execute_native(
    gpu: &native::Gpu,
    compiled: &Compiled,
    program: &mut Program,
    log: &LoftLog,
    hash: [u8; 32],
    check_trace: bool,
) -> Result<Outcome> {
    if log.device_signature != gpu.device_signature {
        return Err(device(
            "cross-device replay needs a numeric tolerance reference",
        ));
    }
    if log.dt_s.to_bits() != DT.to_bits() || log.initial_seed != 0 || log.checkpoints_every != 0 {
        return Err(device(
            "native snapshots require 120 Hz, seed zero and no checkpoints",
        ));
    }
    let mut ticks = log.ticks(hash).map_err(usage)?;
    let end = log.validate().map_err(usage)?;
    let schema = super::artifact::parameters(program, compiled).map_err(Failure::Compile)?;
    let prog = compiled.prog();
    let mut params: BTreeMap<_, _> = live_names(&prog)
        .map(|name| (name.to_owned(), 0.))
        .collect();
    if log.params.len() != params.len() {
        return Err(usage("parameter snapshot differs from the scene"));
    }
    for (name, value) in &log.params {
        update(&schema, &mut params, name, *value)?;
    }
    apply_inputs(
        &schema,
        &mut params,
        ticks.next().expect("validated tick zero").1,
    )?;
    // A snapshot starts from the initial rows. A later update needs a resumable motion dispatch.
    if log
        .inputs
        .iter()
        .any(|e| e.tick != 0 && e.input != Input::End)
    {
        return Err(device(
            "native snapshot cannot apply an input after tick zero",
        ));
    }
    for (i, name) in live_names(&prog).enumerate() {
        program.params[(3 + i) * 4] = params[name];
    }
    let mut missed = None;
    let mut samples = Vec::new();
    for tick in (if check_trace { 0 } else { end })..=end {
        let seconds = tick as f32 * DT;
        if (f64::from(seconds) * 120.).round() != f64::from(tick) {
            return Err(device("native snapshot time cannot represent this tick"));
        }
        samples = native::sample_motion(gpu, program, seconds).map_err(device)?;
        for (name, verdict) in gates::check_records(program, &samples) {
            if !matches!(verdict, Verdict::Held { .. }) && missed.is_none() {
                missed = Some(format!("tick {tick} {name}: {verdict}"));
            }
        }
    }
    let mut tables = compiled.tables.clone();
    for sample in &samples {
        let rows = &mut tables[prog.kind_index(&sample.kind)];
        for (name, values) in &sample.fields {
            let field = column(&prog, &sample.kind, name)?;
            if values.len() != rows.len() {
                return Err(device("incomplete native state readback"));
            }
            for (row, value) in rows.iter_mut().zip(values) {
                row[field] = RowVal::F(*value);
            }
        }
    }
    Ok(Outcome {
        bytes: state_bytes(tables),
        samples,
        params,
        missed,
    })
}

fn update(
    schema: &[ParamMeta],
    params: &mut BTreeMap<String, f32>,
    name: &str,
    value: [f32; 4],
) -> Result<()> {
    if value[1..].iter().any(|&x| x != 0.) {
        return Err(usage("this scene backend accepts scalar parameter values"));
    }
    let slot = params
        .get_mut(name)
        .ok_or_else(|| usage(format!("unknown live parameter {name}")))?;
    let meta = schema
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| device("missing parameter metadata"))?;
    *slot = meta
        .range
        .map_or(value[0], |(lo, hi)| value[0].clamp(lo[0], hi[0]));
    Ok(())
}

fn apply_inputs(
    schema: &[ParamMeta],
    params: &mut BTreeMap<String, f32>,
    events: &[LoggedInput],
) -> Result<()> {
    for e in events {
        match &e.input {
            Input::Level { name, value } | Input::Param { name, value } => {
                update(schema, params, name, *value)?
            }
            Input::Event { .. } | Input::PosePacket { .. } => {
                return Err(device(
                    "this scene backend has no event or pose packet port",
                ));
            }
            Input::End => unreachable!("terminal item is excluded from ticks"),
        }
    }
    Ok(())
}

fn step(ev: &mut Eval<'_, '_, f32>, src: &Sources) -> Result<()> {
    for pass in &ev.p.schedule.expect("compiled schedule").passes {
        if pass.verb.name != "run_laws" {
            continue;
        }
        for arg in &pass.args {
            let ExprKind::Name(name) = &arg.value.kind else {
                return Err(device("unsupported law entry"));
            };
            let law = &ev.p.laws[name.as_str()].f;
            let [row, dt] = law.params.as_slice() else {
                return Err(device("unsupported law signature"));
            };
            let Some(TyExpr::Name(kind)) = &row.ty else {
                return Err(device("law row type is missing"));
            };
            let table = ev.p.kind_index(&kind.name);
            for index in 0..ev.tables[table].len() {
                ev.bind(&row.name.name, Val::Row(table, index));
                ev.bind(&dt.name.name, Val::S(DT));
                let mut locals = 2;
                for stmt in &law.body.stmts {
                    let diag = |d: loft_ir::Diag| device(d.render(src));
                    match stmt {
                        Stmt::Let {
                            mutable: false,
                            name,
                            value,
                            ..
                        } => {
                            let value = ev.expr(value).map_err(diag)?;
                            ev.bind(&name.name, value);
                            locals += 1;
                        }
                        Stmt::Assign {
                            target,
                            op: None,
                            value,
                        } => {
                            let ExprKind::Member(base, field) = &target.kind else {
                                return Err(device("expected a row write"));
                            };
                            if !matches!(&base.kind, ExprKind::Name(n) if n == &row.name.name) {
                                return Err(device("expected a write to the law's row"));
                            }
                            let column = column(ev.p, &kind.name, &field.name)?;
                            let value = ev.expr(value).map_err(diag)?;
                            ev.tables[table][index][column] =
                                concrete(&value, target.span).map_err(diag)?;
                        }
                        _ => {
                            return Err(device(format!(
                                "{} uses statements outside the row-local twin adapter",
                                law.name.name
                            )));
                        }
                    }
                }
                for _ in 0..locals {
                    ev.unbind();
                }
            }
        }
    }
    Ok(())
}

fn column(prog: &Prog<'_>, kind: &str, field: &str) -> Result<usize> {
    prog.kinds[kind]
        .row
        .iter()
        .position(|f| f.name.name == field)
        .ok_or_else(|| device(format!("missing {kind}.{field}")))
}

fn snapshots(
    ev: &mut Eval<'_, '_, f32>,
    program: &Program,
    src: &Sources,
) -> Result<Vec<MotionSample>> {
    program
        .dispatches
        .iter()
        .map(|step| {
            let (kind, _, _, _) = step
                .input
                .view(&program.kinds)
                .ok_or_else(|| device("missing row table"))?;
            let table = ev.p.kind_index(kind);
            let fields = step
                .fields
                .iter()
                .map(|(name, _)| {
                    let column = column(ev.p, kind, name)?;
                    let values = ev.tables[table]
                        .iter()
                        .map(|row| match row[column] {
                            RowVal::F(v) => Ok(v),
                            _ => Err(device("snapshot write must be a scalar float")),
                        })
                        .collect::<Result<_>>()?;
                    Ok((name.clone(), values))
                })
                .collect::<Result<_>>()?;
            let checks = step
                .checks
                .iter()
                .map(|name| {
                    let f = ev.p.checks[name.as_str()];
                    let values = (0..ev.tables[table].len())
                        .map(|row| {
                            let v = ev
                                .call_fn(f, vec![Val::Row(table, row)], Span::NONE)
                                .map_err(|d| device(d.render(src)))?;
                            if let Val::V(v, _) = v {
                                return v
                                    .try_into()
                                    .map_err(|_| device("expected a tag and check margin"));
                            }
                            Err(device("missing check record"))
                        })
                        .collect::<Result<_>>()?;
                    Ok((name.clone(), values))
                })
                .collect::<Result<_>>()?;
            Ok(MotionSample {
                kind: kind.into(),
                fields,
                checks,
            })
        })
        .collect()
}

fn adapt_checks(compiled: &mut Compiled) -> Result<()> {
    let id = compiled.types.len() as u32;
    compiled.types.push(Ty::F32);
    for item in compiled.modules.iter_mut().flat_map(|m| &mut m.items) {
        if let Item::Check(check) = item {
            if let Some(stmt @ Stmt::Expr(_)) = check.body.stmts.last_mut()
                && let Stmt::Expr(expr) = stmt
            {
                *stmt = Stmt::Return(Some(expr.clone()), expr.span);
            }
            adapt_returns(&mut check.body, id)?;
        }
    }
    Ok(())
}

fn adapt_returns(block: &mut loft_parse::ast::Block, id: u32) -> Result<()> {
    for stmt in &mut block.stmts {
        match stmt {
            Stmt::Return(Some(expr), _) => adapt_verdict(expr, id)?,
            Stmt::If {
                then, otherwise, ..
            } => {
                adapt_returns(then, id)?;
                if let Some(otherwise) = otherwise {
                    adapt_returns(otherwise, id)?;
                }
            }
            Stmt::Each { body, .. } | Stmt::For { body, .. } => adapt_returns(body, id)?,
            _ => {}
        }
    }
    Ok(())
}

fn adapt_verdict(expr: &mut Expr, id: u32) -> Result<()> {
    match &mut expr.kind {
        ExprKind::If {
            then, otherwise, ..
        } => {
            adapt_returns(&mut then.body, id)?;
            adapt_returns(&mut otherwise.body, id)?;
            adapt_verdict(&mut then.value, id)?;
            adapt_verdict(&mut otherwise.value, id)
        }
        ExprKind::Call { callee, args } => {
            let value = match callee.name.as_str() {
                "held" => 1.,
                "missed" => 2.,
                "unknown" => 3.,
                _ => return Err(device(format!("unsupported verdict {}", callee.name))),
            };
            // The expression twin understands vec2; use the same tag/margin
            // encoding as the compiler's unchanged snapshot shader.
            let number = |value: f64| Arg {
                name: None,
                value: Expr {
                    id,
                    span: expr.span,
                    kind: ExprKind::Num {
                        value,
                        float: true,
                        text: value.to_string(),
                        suffix: NumSuffix::None,
                    },
                },
            };
            if args.is_empty() {
                args.push(number(0.));
            }
            args.insert(0, number(value));
            callee.name = "vec2".into();
            Ok(())
        }
        _ => Err(device("expected an explicit authored verdict")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCENE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/l5g/scramble.loft");

    #[test]
    fn native_scramble_record_replays_every_state_bit() {
        let src = sources(Path::new(SCENE)).unwrap();
        let (mut program, _, code) =
            loft_passes::compile(&src, &[]).unwrap_or_else(|e| panic!("{}", e.render(&src)));
        let hash = bundle_hash(&program, &code).unwrap();
        let gpu = native::gpu().unwrap();
        let mut log = initial_log(hash, &code.prog(), &program);
        log.device_signature = gpu.device_signature.clone();
        log.record(
            0,
            Input::Param {
                name: "scramble_speed".into(),
                value: [-0.3, 0., 0., 0.],
            },
        )
        .unwrap();
        log.record(481, Input::End).unwrap();
        let original = execute_native(&gpu, &code, &mut program, &log, hash, false)
            .unwrap()
            .bytes;
        assert_eq!(original.len(), 32);
        log.expected_final_identity = Some(identity(&original));
        let loaded = LoftLog::decode(&log.encode().unwrap()).unwrap();
        let (mut fresh, _, code) = loft_passes::compile(&src, &[]).unwrap();
        let replay = execute_native(&gpu, &code, &mut fresh, &loaded, hash, false).unwrap();
        assert_eq!(original, replay.bytes);
        loaded.verify_identity(&replay.bytes).unwrap();
        let mut other = loaded.clone();
        other.inputs[0].input = Input::Param {
            name: "scramble_speed".into(),
            value: [0.3, 0., 0., 0.],
        };
        let changed = execute_native(&gpu, &code, &mut fresh, &other, hash, false).unwrap();
        assert_ne!(
            original, changed.bytes,
            "logged input must drive the device state"
        );
    }

    #[test]
    fn scramble_replays_every_state_bit() {
        let src = sources(Path::new(SCENE)).unwrap();
        let (program, _, mut code) =
            loft_passes::compile(&src, &[]).unwrap_or_else(|d| panic!("{}", d.render(&src)));
        let artifact = super::super::artifact::lower(&program, &code).unwrap();
        loft_runner::bundle::validate_wgsl(&artifact).unwrap();
        let packed = loft_runner::bundle::encode(&artifact);
        assert_eq!(
            packed,
            loft_runner::bundle::encode(&loft_runner::bundle::decode(&packed).unwrap())
        );
        let hash = identity(&packed);
        let mut log = initial_log(hash, &code.prog(), &program);
        adapt_checks(&mut code).unwrap();

        let speed = |v| [v, 0., 0., 0.];
        let name = || "scramble_speed".into();
        log.record(
            1,
            Input::Level {
                name: name(),
                value: speed(-0.6),
            },
        )
        .unwrap();
        log.record(
            80,
            Input::Param {
                name: name(),
                value: speed(-0.4),
            },
        )
        .unwrap();
        log.record(
            80,
            Input::Param {
                name: name(),
                value: speed(-0.3),
            },
        )
        .unwrap();
        log.record(481, Input::End).unwrap();
        let original = execute(&code, &program, &src, &log, hash).unwrap();
        assert!(original.missed.is_none());
        assert_eq!(
            original.params["scramble_speed"].to_bits(),
            (-0.3_f32).to_bits()
        );
        let mut mutant = log.clone();
        mutant.inputs.swap(1, 2);
        assert!(mutant.validate().is_err());
        mutant.inputs.remove(1);
        let different = execute(&code, &program, &src, &mutant, hash).unwrap();
        assert_ne!(original.bytes, different.bytes);

        log.expected_final_identity = Some(identity(&original.bytes));
        let binary = log.encode().unwrap();
        let replay = LoftLog::decode(&binary).unwrap();
        assert_eq!(replay.encode().unwrap(), binary);
        let json = serde_json::to_vec_pretty(&Json(log.clone())).unwrap();
        let Json(json_log) = serde_json::from_slice(&json).unwrap();
        for replay in [replay, json_log] {
            let replayed = execute(&code, &program, &src, &replay, hash).unwrap();
            assert!(replayed.missed.is_none());
            assert_eq!(original.bytes.len(), replayed.bytes.len());
            let bits: u32 = original
                .bytes
                .iter()
                .zip(&replayed.bytes)
                .map(|(&a, &b)| (a ^ b).count_ones())
                .sum();
            assert_eq!(bits, 0);
            replay.verify_identity(&replayed.bytes).unwrap();
            assert!(replay.verify_identity(&different.bytes).is_err());
            println!(
                "481 ticks, {} bytes of state, {bits} differing bits",
                original.bytes.len()
            );
        }
        if let Ok(path) = std::env::var("LOFT_L5G_FIXTURE_OUT") {
            fs::write(path, json).unwrap();
        }
    }
}
