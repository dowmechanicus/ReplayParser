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

pub fn parse_file(file_path: String) -> Option<String> {
    let path = Path::new(&file_path);
    let replay = match parse_replay(&path) {
        Ok(r) => Some(r),
        Err(e) => {
            println!("error: {}", e);
            None
        }
    };
    let json = serde_json::to_string_pretty(&replay).unwrap();
    Some(json.clone())
}
