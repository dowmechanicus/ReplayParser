#[derive(Clone, Debug, Serialize)]
pub struct Message {
    pub tick: u32,
    pub sender: String,
    pub receiver: String,
    pub body: String,
    pub player_id: u8,
}
