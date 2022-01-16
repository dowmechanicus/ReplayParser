use super::*;

#[derive(Debug, Default, Serialize)]
pub struct UnknownAction {
    pub data: Vec<u8>,
    pub source: String,
    pub tick: u32,
    pub player_id: u8,
}
impl ParseAction for UnknownAction {}
impl<'a> From<ActionData<'a>> for UnknownAction {
    fn from(action_data: ActionData) -> Self {
        let (data, tick) = action_data;
        Self {
            data: data.clone(),
            source: format!("{:#X}", data[7]),
            tick,
            player_id: get_player_id(data),
        }
    }
}
