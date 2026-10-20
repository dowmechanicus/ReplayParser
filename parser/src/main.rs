use std::path::PathBuf;
use std::process;

use clap::Parser;
use parser_lib::pipeline::{Chain, GameData};

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

    let game = resolve_module(args.module)
        .map(|p| GameData::open(&p))
        .transpose()?;
    match parser_lib::parse_raw(file) {
        Ok(mut replay) => {
            Chain::with_defaults(game.as_ref()).run(&mut replay)?;
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
