use crate::{actions::Action, chunky::Chunk, message::Message, metadata::MetadataScan};

/// An outer replay record whose type is not understood by this parser.
/// Retaining the type and payload keeps future record kinds inspectable.
#[derive(Debug, Serialize)]
pub struct RawRecord {
    pub record_type: u32,
    pub payload: Vec<u8>,
}

/// A synchronization record's generic prefix and all of its action bundles.
#[derive(Clone, Debug, Serialize)]
pub struct SyncRecord {
    pub marker: u8,
    pub tick: u32,
    pub counter: u32,
    pub unknown: u32,
    pub bundles: Vec<ActionBundle>,
}

/// A bundle's opaque metadata, redundant length echo, and lossless commands.
#[derive(Clone, Debug, Serialize)]
pub struct ActionBundle {
    pub metadata: u64,
    pub declared_size: u32,
    pub length_echo: u8,
    pub commands: Vec<Action>,
}

#[derive(Default, Serialize)]
pub struct ReplayInfo {
    pub name: String,
    pub mod_chksum: u32,
    pub mod_version: u32,
    pub md5: String,
    pub date: String,
    pub ticks: u32,
    /// Lossless bounded header/chunk scan retained alongside legacy chunks.
    pub metadata: Option<MetadataScan>,
    pub sync_records: Vec<SyncRecord>,
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
