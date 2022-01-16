use super::{ActionData, ParseAction};

#[derive(Debug, Serialize)]
pub struct BuildingAction {
    pub building_id: u8,
}
impl ParseAction for BuildingAction {}
impl<'a> From<ActionData<'a>> for BuildingAction {
    fn from(action_data: ActionData) -> Self {
        let (data, _) = action_data;

        if data.len() > 13 {
            Self {
                building_id: data[13],
            }
        } else {
            Self {
                building_id: 0
            }
        }
    }
}
