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
        82 => "Tyranids HQ (Tier 2)",
        83 => "Tyranids HQ (Tier 3)",
        190 => "Space Marines HQ (Tier 2)",
        191 => "Space Marines HQ (Tier 3)",
        20 => "Orks HQ (Tier 2)",
        21 => "Orks HQ (Tier 3)",
        52 => "Imperial Guard HQ (Tier 2)",
        54 => "Imperial Guard HQ (Tier 3)",
        40 => "Chaos Space Marines HQ (Tier 2)",
        41 => "Chaos Space Marines HQ (Tier 3)",
        124 => "Eldar HQ (Tier 2)",
        125 => "Eldar HQ (Tier 3)",
        36 | 107 | 111 | 147 | 192 => "Power Node",
        31 | 98 | 102 | 142 | 189 => "Power Generator",
        _ => "Unknown building"
    }
}
