//! loft-lower, as tools/port/diff.ts runs it on one entry:
//!
//!   loft-lower compile <input dir> --out <dir>             the whole port: input/job.bin (and input/baked) to
//!                                                          <dir>/probe/*.txt, <dir>/compile.json, <dir>/artefact/
//!   loft-lower lower <compile.json> <baked dir> --out <dir>  the lower lanes alone, from the TS compiler's Artefact
//!
//! Each stage writes what it made before the next runs, so a lane sees its own sections compared while later lanes are
//! still unported. The last stderr line of a failure is `loft: <kind>: <head>` with the CLI's exit code.
use loft_lower::{Error, compile, draw, eval, graph, input, lower};
use std::path::{Path, PathBuf};
use std::{fs, process};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // The canonical print recurses once per graph level (loft-ir print.rs): a deep scene needs a large stack.
    let run = std::thread::Builder::new().stack_size(1 << 30).spawn(move || run(&args)).expect("a thread");
    match run.join().expect("loft-lower panicked") {
        Ok(()) => {}
        Err(e) => {
            eprintln!("{e}");
            process::exit(e.code());
        }
    }
}

fn usage() -> Error {
    Error::Internal(
        "usage: loft-lower compile <input dir> --out <dir> | lower <compile.json> <baked dir> --out <dir>".into(),
    )
}

fn run(args: &[String]) -> Result<(), Error> {
    let out = match args.iter().position(|a| a == "--out") {
        Some(k) => PathBuf::from(args.get(k + 1).ok_or_else(usage)?),
        None => return Err(usage()),
    };
    fs::create_dir_all(&out).map_err(|e| Error::Internal(format!("{}: {e}", out.display())))?;
    match args.first().map(String::as_str) {
        Some("compile") => {
            let dir = Path::new(args.get(1).ok_or_else(usage)?);
            let bytes =
                fs::read(dir.join("job.bin")).map_err(|e| Error::Input(format!("{}/job.bin: {e}", dir.display())))?;
            let job = input::decode(&bytes)?;
            let probes = out.join("probe");
            write(&probes, "wire.txt", &input::print(&job)?)?;
            // A probe whose lane has not landed is left unwritten: its section is absent and fails in that lane's scope.
            for (name, probe) in
                [("graph.txt", graph::probe as fn(&input::Input) -> Result<String, Error>), ("eval.txt", eval::probe)]
            {
                match probe(&job) {
                    Ok(text) => write(&probes, name, &text)?,
                    Err(Error::Unported { .. }) => {}
                    Err(e) => return Err(e),
                }
            }
            for (name, text) in draw::tape(&job) {
                write(&probes, &format!("{name}.txt"), &text)?;
            }
            let mut fields = compile::Fields::new();
            let done = compile::compile(&job, &mut fields);
            let compiled =
                serde_json::Value::Object(fields.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()).to_string();
            write(&out, "compile.json", &compiled)?;
            done?;
            lower::lower(&compiled, &dir.join("baked"), &out.join("artefact"))
        }
        Some("lower") => {
            let file = Path::new(args.get(1).ok_or_else(usage)?);
            let baked = Path::new(args.get(2).ok_or_else(usage)?);
            let text = fs::read_to_string(file).map_err(|e| Error::Input(format!("{}: {e}", file.display())))?;
            lower::lower(&text, baked, &out.join("artefact"))
        }
        _ => Err(usage()),
    }
}

fn write(dir: &Path, name: &str, text: &str) -> Result<(), Error> {
    fs::create_dir_all(dir)
        .and_then(|()| fs::write(dir.join(name), text))
        .map_err(|e| Error::Internal(format!("{}/{name}: {e}", dir.display())))
}
