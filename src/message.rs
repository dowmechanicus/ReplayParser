#[derive(Serialize)]
pub struct Message {
    pub tick: u32,
    pub sender: String,
    pub receiver: String,
    pub body: String,
}
