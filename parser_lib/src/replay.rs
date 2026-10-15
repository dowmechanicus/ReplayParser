use crate::{
    actions::Action, chunky::Chunk, message::Message, metadata::MetadataScan, stats::PlayerStats,
};

/// An outer replay record whose type is not understood, or whose known
/// payload could not be semantically decoded. Retaining the type and payload
/// keeps future record kinds inspectable and avoids silent loss.
#[derive(Debug, Serialize)]
pub struct RawRecord {
    pub record_type: u32,
    pub payload: Vec<u8>,
    pub error: Option<String>,
}

/// A synchronization record's generic prefix and all of its action bundles.
#[derive(Clone, Debug, Serialize)]
pub struct SyncRecord {
    pub marker: u8,
    pub tick: u32,
    pub counter: u32,
    pub unknown: u32,
    /// Exact payload bytes, retained so fields added by future game versions
    /// remain available even when this parser does not interpret them.
    pub raw_payload: Vec<u8>,
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

/// Module whose catalogues were used to resolve blueprint names, and their sizes.
#[derive(Debug, Default, Serialize)]
pub struct BlueprintSource {
    pub module: String,
    pub ebp_count: usize,
    pub sbp_count: usize,
    pub upgrade_count: usize,
    pub ability_count: usize,
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
    #[serde(skip_serializing)]
    pub metadata: Option<MetadataScan>,
    #[serde(skip_serializing)]
    pub sync_records: Vec<SyncRecord>,
    pub game: Chunk,
    pub map: Chunk,
    pub players: Vec<Chunk>,
    pub observers: Vec<Chunk>,
    pub messages: Vec<Message>,
    /// All decoded commands, including opcodes omitted from the legacy
    /// filtered `actions` view.
    pub commands: Vec<Action>,
    /// Legacy filtered view retained for existing consumers.
    pub actions: Vec<Action>,
    /// Per-player order counts, derived from `commands` once seats are assigned.
    pub stats: Vec<PlayerStats>,
    pub unknown_records: Vec<RawRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blueprints: Option<BlueprintSource>,
}
