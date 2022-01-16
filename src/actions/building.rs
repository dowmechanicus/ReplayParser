use super::{ActionData, ParseAction};

#[derive(Debug, Serialize)]
pub struct BuildingAction {
    pub building_id: u8,
    pub building_name: String,
}
impl ParseAction for BuildingAction {}
impl<'a> From<ActionData<'a>> for BuildingAction {
    fn from(action_data: ActionData) -> Self {
        let (data, _) = action_data;

        if data.len() > 13 {
            Self {
                building_id: data[13],
                building_name: get_building_by_item_id(data[13]).to_string(),
            }
        } else {
            Self {
                building_id: 0,
                building_name: "".to_string(),
            }
        }
    }
}

fn get_building_by_item_id(id: u8) -> &'static str {
    match id {
        107 | 192 => "Power Node",
        98 | 189 => "Power Generator",
        _ => "Unknown building"
    }
}
