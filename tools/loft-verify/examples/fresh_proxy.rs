//! Study instrument: replace the primary raster box and march entry with the
//! compiler's inner interval. Keep the culler, sun camera and shadow shader.
use loft_cli::{native, sources, write_png};
use loft_passes::{compile, kind_space};
use std::{env, fs, path::Path};

fn main() -> Result<(), String> {
    let args: Vec<_> = env::args().skip(1).collect();
    let [scene, width, height, dest] = &args[..] else {
        return Err("expected scene, width, height, output directory".into());
    };
    let size = (
        width.parse().map_err(|_| "width")?,
        height.parse().map_err(|_| "height")?,
    );
    let dest = Path::new(dest);
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let src = sources(Path::new(scene))?;
    let (mut program, _, compiled) = compile(&src, &[]).map_err(|e| e.render(&src))?;
    let decls = compiled.prog();
    let mut summary = vec![];
    for (id, k) in program.kinds.iter().enumerate() {
        let m = &mut program.modules[k.module];
        let Some((_, tail)) = m.wgsl.split_once("let g = bitcast<u32>(luts[") else {
            continue;
        };
        let (base, _) = tail
            .split_once("u + kp.kind_id]")
            .ok_or("missing guard table")?;
        let header: usize = base.parse().map_err(|_| "invalid guard header")?;
        if kind_space(decls.kinds[k.name.as_str()]) != "World" {
            return Err(format!(
                "this experiment requires world-space queries for {}",
                k.name
            ));
        }
        let first = program.luts[header + id].to_bits() as usize;
        let box_words = &program.luts[first..first + 10 * k.n_rows as usize];
        summary.push(
            serde_json::json!({"kind":k.name,"guards":box_words, "outer":program.instances.iter()
            .filter(|r|r.kind == id as u32).map(|r| [r.lo,r.hi]).collect::<Vec<_>>()}),
        );
        let helper = format!(
            r#"
fn experiment_row(i: u32) -> Row {{
  var row = load_row(i);
  let g = bitcast<u32>(luts[{header}u + kp.kind_id]) + 10u*i;
  let center = vec3f(luts[g+6u],luts[g+7u],luts[g+8u]);
  if frame.cold.y == 0u && distance(frame.eye.xyz,center) <= 32.0 && luts[g+9u] > 0.0 {{
    row.loft_lo = max(row.loft_lo, vec3f(luts[g],luts[g+1u],luts[g+2u]));
    row.loft_hi = min(row.loft_hi, vec3f(luts[g+3u],luts[g+4u],luts[g+5u]));
  }}
  return row;
}}
"#
        );
        m.wgsl = m
            .wgsl
            .replace("@vertex fn vs(", &(helper + "\n@vertex fn vs("));
        for (from, to) in [
            ("let row = load_row(r);", "let row = experiment_row(r);"),
            (
                "let row = load_row(v.row);",
                "let row = experiment_row(v.row);",
            ),
        ] {
            if !m.wgsl.contains(from) {
                return Err(format!("missing {from} in {}", k.name));
            }
            m.wgsl = m.wgsl.replacen(from, to, 1);
        }
        let a = m
            .wgsl
            .find("  // Restrict only the exit.")
            .ok_or("missing guard")?;
        let b = a + m.wgsl[a..].find("  let step = ").ok_or("missing march")?;
        m.wgsl.replace_range(a..b, "");
        m.naga = naga::front::wgsl::parse_str(&m.wgsl).map_err(|e| e.emit_to_string(&m.wgsl))?;
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::default(),
        )
        .validate(&m.naga)
        .map_err(|e| e.to_string())?;
    }
    fs::write(
        dest.join("bounds.json"),
        serde_json::to_vec_pretty(&summary).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    let g = native::gpu()?;
    let loaded = native::Loaded::new(&g, &program, size, None)?;
    for i in 0..2 {
        g.triangle_control(size)?;
        let before = g.load_control(size)?[0];
        let times = (0..30)
            .map(|i| i as f32 / 30.)
            .chain((0..120).map(|i| i as f32 * (1.0 / 60.0)));
        let all = loaded.profile_frames(&g, times, false)?;
        let after = g.load_control(size)?[0];
        let timed = &all[30..];
        let d = serde_json::json!({"adapter":g.adapter, "passes":program.passes.iter().map(|p|format!("{p:?}")).collect::<Vec<_>>(),
            "gpu_frame_ms":timed.iter().map(|f|f.gpu_ms).collect::<Vec<_>>(),
            "gpu_pass_ms":timed.iter().map(|f|&f.pass_ms).collect::<Vec<_>>(),
            "load_control_before_p50_ms":before, "load_control_after_p50_ms":after,
            "contended":before.max(after)>native::QUIET_CONTROL_P50_MS});
        fs::write(
            dest.join(format!("profile-{i}.json")),
            serde_json::to_vec_pretty(&d).unwrap(),
        )
        .map_err(|e| e.to_string())?;
        if !d["contended"].as_bool().unwrap() {
            break;
        }
    }
    for i in 0..120 {
        loaded.frame(&g, (f64::from(i) / 60.) as f32);
        write_png(
            &dest.join(format!("frame_{i:03}.png")),
            size,
            &loaded.pixels(&g)?,
        )?;
    }
    Ok(())
}
