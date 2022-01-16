use super::*;

#[derive(Debug, Serialize)]
pub struct PurchaseUnitAction {
    pub unit_id: u8,
    pub unit_name: String,
}
impl ParseAction for PurchaseUnitAction {}
impl<'a> From<ActionData<'a>> for PurchaseUnitAction {
    fn from(action_data: ActionData) -> Self {
        let (data, _) = action_data;
        if data.len() > 13 {
            Self {
              unit_id: data[10],
              unit_name: get_unit_by_item_id(data[13]).to_string(),
            }
        } else {
            Self {
              unit_id: 0,
              unit_name: "".to_string(),
            }
        }
    }
}

fn get_unit_by_item_id(id: u8) -> &'static str {
  match id {
      141 => "Dire Avengers",
      137 => "Howling Banshees",
      146 => "Rangers",
      144 => "Shuriken Platform",
      138 => "Dark Reapers",
      148 => "Warp Spiders",
      140 => "Fire Dragons",
      153 => "Wraithlord",
      151 => "Falcon",
      149 => "Wraithguard",
      152 => "Fire Prism",
      143 => "D-Cannon",
      145 => "Avatar",
      147 => "Seer Council",
      191 => "Catachan Devils",
      193 => "Heavy Weapons Squad",
      219 => "Scouts",
      221 => "Tactical Marines",
      216 => "Devastator Squad",
      215 => "Assault Marines",
      217 => "Plasma Devastators",
      225 => "Dreadnought",
      228 => "Razorback",
      218 => "Librarian",
      230 => "Whirlwind",
      227 => "Predator Tank",
      226 => "Landraider Redeemer",
      _ => "unknown unit",
  }
}
