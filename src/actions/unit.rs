use super::*;

#[derive(Debug, Serialize)]
pub struct UnitAction {
    pub action_type: u8,
    pub unit_name: String,
    pub unit_id: (u8, u8),
    pub player_id: u8,
    pub tick: u32,
}
impl ParseAction for UnitAction {}
impl<'a> From<ActionData<'a>> for UnitAction {
    fn from(action_data: ActionData) -> Self {
        let (data, tick) = action_data;
        if data.len() > 13 {
            Self {
                action_type: data[1],
                unit_name: get_unit_by_item_id(data[13]).to_string(),
                unit_id: (data[10], data[11]),
                player_id: get_player_id(data),
                tick: tick,
            }
        } else {
            if data.len() > 10 && data.len() > 11 {
                Self {
                    action_type: data[1],
                    unit_name: "unknown".to_string(),
                    unit_id: (data[10], data[11]),
                    player_id: get_player_id(data),
                    tick: tick,
                }
            } else {
                // This case may arise when using alt + x
                Self {
                    action_type: data[1],
                    unit_name: "unknown".to_string(),
                    unit_id: (0, 0),
                    player_id: get_player_id(data),
                    tick: tick,
                }
            }
        }
    }
}

fn get_wargear_by_id(id: u8) -> &'static str {
    match id {
        197 => "Assault Marine Sergeant",
        216 => "Scout - Elite training",
        217 => "Scout - Sergeant",
        218 => "Scout - Shotguns",
        227 => "Vanguard Veteran Squad",
        231 => "Force Commander - Iron Halo",
        240 => "Force Commander - Alacrity armor",
        252 => "Force Commander - Power Sword",
        241 => "Terminator Force Commander",
        _ => "Unknown wargear",
    }
}

fn get_unit_by_item_id(id: u8) -> &'static str {
    match id {
        137 => "Howling Banshees",
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
