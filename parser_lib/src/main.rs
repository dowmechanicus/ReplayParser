use std::env;
use std::path::Path;

extern crate byteorder;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate crypto;

mod chunky;
mod message;
mod parse;
mod replay;
use parse::*;
mod actions;

fn main() {
    if let Some(arg) = env::args().nth(1) {
        let path = Path::new(&arg);
        let replay = match parse_replay(&path) {
            Ok(r) => r,
            Err(e) => {
                println!("error: {}", e);
                std::process::exit(1);
            }
        };
        let json = serde_json::to_string_pretty(&replay).unwrap();
        println!("{}", json);
    } else {
        println!("must supply a file");
    }
}
