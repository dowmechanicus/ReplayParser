use crate::{actions::ActionType, chunky::Chunk, message::Message};

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
    pub actions: Vec<ActionType>,
}
