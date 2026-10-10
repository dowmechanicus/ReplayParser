use std::env;
use std::path::Path;
use std::process;

use parser_lib;
use parser_lib::attrib::AttribMount;
use parser_lib::blueprints::BlueprintCatalogues;
use parser_lib::module::ModuleFile;
use parser_lib::names::{DisplayNames, DEFAULT_LANGUAGE};

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
    let module_file = match ModuleFile::open(module_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("could not read module {}: {}", module, e);
            process::exit(1);
        }
    };
    let mount = match AttribMount::open(install_dir, &module_file) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("could not read module {}: {}", module, e);
            process::exit(1);
        }
    };
    let catalogue = BlueprintCatalogues::from_mount(&mount);
    match parser_lib::parse_raw(file) {
        Ok(mut replay) => {
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
}
