use super::{ActionData, ParseAction, get_player_id};

#[derive(Debug, Serialize)]
pub struct GlobalAction {
  pub action_type: u8,
  pub player_id: u8,
  pub source: String,
  pub tick: u32,
  pub data: Vec<u8>,
}

impl ParseAction for GlobalAction {}
impl<'a> From<ActionData<'a>> for GlobalAction {
  fn from(action_data: ActionData) -> Self {
    let (data, tick) = action_data;
    Self {
        action_type: data[1],
        data: data.clone(),
        source: format!("{:#X}", data[7]),
        player_id: get_player_id(data),
        tick,
    }
  }
}
