use color_eyre::Result;
use std::path::Path;

extern crate byteorder;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate crypto;

pub mod actions;
pub mod chunky;
pub mod message;
mod parse;
pub mod replay;
use parse::*;

pub fn parse_file(file_path: String) -> Option<String> {
    let path = Path::new(&file_path);
    let replay = match parse_replay(path) {
        Ok(r) => Some(r),
        Err(e) => {
            println!("error: {}", e);
            None
        }
    };
    let json = serde_json::to_string_pretty(&replay).unwrap();
    Some(json)
}

pub fn parse_raw(file_path: String) -> Result<replay::ReplayInfo> {
    let path = Path::new(&file_path);
    let replay_info = parse_replay(path)?;

    Ok(replay_info)
}
