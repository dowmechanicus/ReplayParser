use std::path::{Path, PathBuf};
use std::process;

use clap::Parser;
use parser_lib::attrib::AttribMount;
use parser_lib::blueprints::BlueprintCatalogues;
use parser_lib::module::ModuleFile;
use parser_lib::names::{DisplayNames, DEFAULT_LANGUAGE};

mod discover;

#[derive(Parser)]
struct Args {
    /// Replay file to parse.
    file: PathBuf,
    /// Path to Elite.module, used to resolve blueprint names (auto-located when omitted).
    #[arg(long)]
    module: Option<PathBuf>,
}

fn main() {
    if let Err(e) = run(Args::parse()) {
        eprintln!("{}", e);
        process::exit(1);
    }
}

/// The explicit `--module` path, else Elite.module found through Steam (DOW2_INSTALL first).
fn resolve_module(explicit: Option<PathBuf>) -> Option<PathBuf> {
    explicit.or_else(|| match discover::locate_elite_module() {
        Ok(path) => {
            eprintln!("info: auto-located module: {}", path.display());
            Some(path)
        }
        Err(e) => {
            eprintln!("warning: {}", e);
            None
        }
    })
}

fn run(args: Args) -> Result<(), String> {
    let file = args.file.to_string_lossy().into_owned();

    let Some(module_path) = resolve_module(args.module) else {
        if let Some(json) = parser_lib::parse_file(file) {
            println!("{}", json);
        }
        return Ok(());
    };

    let read_err = |e| format!("could not read module {}: {}", module_path.display(), e);
    let install_dir = module_path.parent().unwrap_or_else(|| Path::new("."));
    let module_file = ModuleFile::open(&module_path).map_err(read_err)?;
    let mount = AttribMount::open(install_dir, &module_file).map_err(read_err)?;
    let catalogue = BlueprintCatalogues::from_mount(&mount);
    match parser_lib::parse_raw(file) {
        Ok(mut replay) => {
            let module = module_path.to_string_lossy();
            parser_lib::resolve_blueprints(&mut replay, &catalogue, &module);
            match DisplayNames::load(&mount, install_dir, &module_file, DEFAULT_LANGUAGE) {
                Ok(names) => parser_lib::resolve_display_names(&mut replay, &names),
                Err(e) => eprintln!("warning: display names unavailable: {}", e),
            }
            match parser_lib::to_json(&replay) {
                Ok(json) => println!("{}", json),
                Err(e) => println!("error: {}", e),
            }
        }
        Err(e) => {
            println!("error: {}", e);
            println!("null");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_module_is_returned_untouched() {
        let path = PathBuf::from("/nonexistent/Elite.module");
        assert_eq!(resolve_module(Some(path.clone())), Some(path));
    }
}
