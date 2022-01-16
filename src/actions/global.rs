use super::{ActionData, ParseAction};

#[derive(Debug, Serialize)]
pub struct GlobalAction {
    pub action_type: u8,
}

impl ParseAction for GlobalAction {}
impl<'a> From<ActionData<'a>> for GlobalAction {
    fn from(action_data: ActionData) -> Self {
        let (data, _) = action_data;
        Self {
            action_type: data[1],
        }
    }
}
