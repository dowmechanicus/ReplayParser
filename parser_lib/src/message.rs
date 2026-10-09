#[derive(Clone, Debug, Serialize)]
pub struct Message {
    pub tick: u32,
    pub wire_kind: u32,
    pub wire_size: u32,
    pub sender: String,
    pub receiver: String,
    pub body: String,
    pub player_id: u8,
    pub player_reserved: [u8; 3],
    pub kind: u32,
    pub local: u32,
    /// Complete bounded chat payload, including the duplicate kind/size and
    /// fields not yet assigned a semantic name.
    pub raw: Vec<u8>,
}
