use std::env;
use std::path::Path;
use std::process;

use parser_lib;
use parser_lib::blueprints::BlueprintCatalogues;

fn main() {
    let mut file = None;
    let mut module = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--module" {
            match args.next() {
                Some(path) => module = Some(path),
                None => {
                    eprintln!("--module requires a path");
                    process::exit(1);
                }
            }
        } else if file.is_none() {
            file = Some(arg);
        }
    }

    let Some(file) = file else {
        println!("No input file specified");
        return;
    };

    let Some(module) = module else {
        match parser_lib::parse_file(file) {
            Some(json) => println!("{}", json),
            None => println!("File could not be parsed"),
        }
        return;
    };

    let module_path = Path::new(&module);
    let install_dir = module_path.parent().unwrap_or_else(|| Path::new("."));
    let catalogue = match BlueprintCatalogues::from_module(install_dir, module_path) {
        Ok(catalogue) => catalogue,
        Err(e) => {
            eprintln!("could not read module {}: {}", module, e);
            process::exit(1);
        }
    };
    match parser_lib::parse_raw(file) {
        Ok(mut replay) => {
            parser_lib::resolve_blueprints(&mut replay, &catalogue, &module);
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
}
