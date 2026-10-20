use std::env;
use std::path::PathBuf;
use std::process;

use parser_lib::pipeline::{Chain, GameData};

/// Replay file (first non-flag argument) and the optional `--module <path>`.
fn parse_args(
    args: impl Iterator<Item = String>,
) -> Result<(Option<String>, Option<PathBuf>), String> {
    let mut file = None;
    let mut module = None;
    let mut args = args;
    while let Some(arg) = args.next() {
        if arg == "--module" {
            module = Some(PathBuf::from(args.next().ok_or("--module needs a path")?));
        } else if file.is_none() {
            file = Some(arg);
        }
    }
    Ok((file, module))
}

fn main() {
    let (file, module) = match parse_args(env::args().skip(1)) {
        Ok(parsed) => parsed,
        Err(e) => {
            println!("error: {}", e);
            process::exit(1);
        }
    };
    let Some(file) = file else {
        println!("must supply a file");
        return;
    };

    let game = match module.as_deref().map(GameData::open).transpose() {
        Ok(game) => game,
        Err(e) => {
            println!("error: {}", e);
            process::exit(1);
        }
    };
    let mut replay = match parser_lib::parse_raw(file) {
        Ok(r) => r,
        Err(e) => {
            println!("error: {}", e);
            process::exit(1);
        }
    };
    if let Err(e) = Chain::with_defaults(game.as_ref()).run(&mut replay) {
        println!("error: {}", e);
        process::exit(1);
    }
    let json = serde_json::to_string_pretty(&replay).unwrap();
    println!("{}", json);
}
