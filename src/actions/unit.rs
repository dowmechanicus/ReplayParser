use super::*;

#[derive(Debug, Serialize)]
pub struct UnitAction {
    pub unit_id: u8,
}
impl ParseAction for UnitAction {}
impl<'a> From<ActionData<'a>> for UnitAction {
    fn from(action_data: ActionData) -> Self {
        let (data, _) = action_data;
        if data.len() > 10 {
            Self { unit_id: data[10] }
        } else {
            Self { unit_id: 0 }
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
        235 => "Techmarine - Refractor Shield",
        240 => "Force Commander - Alacrity armor",
        241 => "Terminator Force Commander",
        242 => "Techmarine - Artificer Armor",
        252 => "Force Commander - Power Sword",
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
