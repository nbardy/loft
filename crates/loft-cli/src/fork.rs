//! Copy a library module and resolve its imports from the destination before publishing it.
use crate::budget::Failure;
use loft_ir::Sources;
use loft_parse::ast::Item;
use loft_passes::source::{Graph, workspace_root};
use std::{fs, io::Write as _, path::{Path, PathBuf}};

const USAGE: &str = "usage: loft fork <std_or_engine_path> <dest.loft> [--force]";

pub(crate) fn run(args: &[String]) -> Result<(), Failure> {
    let force = args.iter().filter(|arg| arg.as_str() == "--force").count();
    let paths = args.iter().filter(|arg| arg.as_str() != "--force").collect::<Vec<_>>();
    let [source, destination] = paths.as_slice() else {
        return Err(Failure::Usage(USAGE.into()));
    };
    if force > 1 || paths.iter().any(|arg| arg.starts_with('-')) {
        return Err(Failure::Usage(USAGE.into()));
    }

    let cwd = std::env::current_dir().map_err(|e| Failure::Usage(e.to_string()))?;
    let root = workspace_root(&cwd)
        .ok_or_else(|| Failure::Usage("cannot find the Loft workspace and its stdlib/".into()))?;
    let library = root.join("stdlib");
    let library = library.canonicalize().map_err(|e| file_error(&library, e))?;
    let input = module_path(&library, source)?;
    let destination = Path::new(destination.as_str());
    let stem = destination.file_stem().and_then(|name| name.to_str()).filter(|name| identifier(name))
        .filter(|_| destination.extension().is_some_and(|ext| ext == "loft"))
        .ok_or_else(|| Failure::Usage("destination must be an importable module name ending in .loft".into()))?;
    let parent = destination.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let parent = parent.canonicalize().map_err(|e| file_error(parent, e))?;
    let output = parent.join(destination.file_name().expect("validated file name"));
    if input == output {
        return Err(Failure::Usage("choose a destination other than the source module".into()));
    }

    let text = fs::read_to_string(&input).map_err(|e| file_error(&input, e))?;
    let copy = rebase_imports(&library, &input, text)?;
    let target_root = workspace_root(&parent)
        .ok_or_else(|| Failure::Usage("cannot find stdlib/ for the destination".into()))?;
    let graph = Graph::text(&target_root, &output, &copy);
    graph.validate_imports().map_err(|d| Failure::Compile(d.render(&graph)))?;

    // Publish only after validation, and check for clobbering atomically.
    let mut pending = tempfile::NamedTempFile::new_in(&parent).map_err(|e| file_error(&output, e))?;
    pending.write_all(copy.as_bytes()).map_err(|e| file_error(&output, e))?;
    let result = if force == 1 {
        pending.persist(&output)
    } else {
        pending.persist_noclobber(&output)
    };
    result.map_err(|e| file_error(&output, e.error))?;
    println!("use self::{stem}");
    Ok(())
}

fn identifier(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|b| b == b'_' || b.is_ascii_alphabetic())
        && bytes.all(|b| b == b'_' || b.is_ascii_alphanumeric())
}

fn module_path(library: &Path, name: &str) -> Result<PathBuf, Failure> {
    let name = name.strip_suffix(".loft").unwrap_or(name).replace("::", "/");
    let relative = name.strip_prefix("stdlib/").unwrap_or(&name);
    if !relative.split('/').all(identifier) {
        return Err(Failure::Usage(format!("invalid stdlib module path `{name}`")));
    }
    let path = library.join(relative).with_extension("loft");
    let path = path.canonicalize().map_err(|e| file_error(&path, e))?;
    if !path.starts_with(library) {
        return Err(Failure::Usage(format!("{} is outside stdlib/", path.display())));
    }
    Ok(path)
}

fn rebase_imports(library: &Path, input: &Path, text: String) -> Result<String, Failure> {
    let mut sources = Sources::default();
    let id = sources.add(input.display().to_string(), text);
    let text = &sources.files[usize::from(id)].1;
    let ast = loft_parse::parse(id, text, &mut 0).map_err(|d| Failure::Compile(d.render(&sources)))?;
    let parent = input.parent().expect("canonical module has a directory");
    let mut prefix = String::from("stdlib");
    for part in parent.strip_prefix(library).expect("stdlib module").components() {
        let name = part.as_os_str().to_str().filter(|name| identifier(name))
            .ok_or_else(|| Failure::Usage(format!("cannot express {} as an import path", parent.display())))?;
        prefix.push_str("::");
        prefix.push_str(name);
    }
    let mut text = std::mem::take(&mut sources.files[usize::from(id)].1);
    // Only the path root changes. Keep the original qualifier, selections and comments.
    for item in ast.items.iter().rev() {
        if let Item::Use(import) = item
            && let Some(first) = import.path.first().filter(|name| name.name == "self")
        {
            text.replace_range(first.span.lo as usize..first.span.hi as usize, &prefix);
        }
    }
    Ok(text)
}

fn file_error(path: &Path, error: std::io::Error) -> Failure {
    Failure::Usage(format!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sibling_imports_keep_their_bindings_after_relocation() {
        let library = Path::new("/workspace/stdlib");
        let input = library.join("engines/local.loft");
        let source = "# self::ink is the local style\nuse self::ink as pigment\nuse self::lighting::sun::{direct}\n";
        let moved = rebase_imports(library, &input, source.into()).unwrap();
        assert_eq!(moved, "# self::ink is the local style\nuse stdlib::engines::ink as pigment\nuse stdlib::engines::lighting::sun::{direct}\n");
    }
}
