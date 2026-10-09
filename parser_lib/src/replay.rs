use crate::{actions::Action, chunky::Chunk, message::Message};

/// An outer replay record whose type is not understood by this parser.
/// Retaining the type and payload keeps future record kinds inspectable.
#[derive(Debug, Serialize)]
pub struct RawRecord {
    pub record_type: u32,
    pub payload: Vec<u8>,
}

#[derive(Default, Serialize)]
pub struct ReplayInfo {
    pub name: String,
    pub mod_chksum: u32,
    pub mod_version: u32,
    pub md5: String,
    pub date: String,
    pub ticks: u32,
    pub game: Chunk,
    pub map: Chunk,
    pub players: Vec<Chunk>,
    pub observers: Vec<Chunk>,
    pub messages: Vec<Message>,
    /// All decoded commands, including opcodes omitted from the legacy
    /// filtered `actions` view.
    pub commands: Vec<Action>,
    pub actions: Vec<Action>,
    pub unknown_records: Vec<RawRecord>,
}
