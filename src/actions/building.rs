use super::{get_player_id, ActionData, ParseAction};

#[derive(Debug, Serialize)]
pub struct BuildingAction {
    pub action_type: u8,
    pub player_id: u8,
    pub tick: u32,
    pub data: Vec<u8>,
}
impl ParseAction for BuildingAction {}
impl<'a> From<ActionData<'a>> for BuildingAction {
    fn from(action_data: ActionData) -> Self {
        let (data, tick) = action_data;
        Self {
            action_type: data[1],
            data: data.clone(),
            tick,
            player_id: get_player_id(data),
        }
    }
}
