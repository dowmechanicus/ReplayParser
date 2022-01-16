use super::*;

#[derive(Debug, Serialize)]
pub struct PurchaseWargearAction {
    pub unit_id: u8,
    pub wargear_id: u8,
    pub wargear_name: String,
}
impl ParseAction for PurchaseWargearAction {}
impl<'a> From<ActionData<'a>> for PurchaseWargearAction {
    fn from(action_data: ActionData) -> Self {
        let (data, _) = action_data;
        if data.len() > 13 {
            Self {
                unit_id: data[10],
                wargear_id: data[13],
                wargear_name: get_wargear_by_item_id(data[13]).to_string(),
            }
        } else {
            Self {
                unit_id: 0,
                wargear_id: data[13],
                wargear_name: "".to_string(),
            }
        }
    }
}

fn get_wargear_by_item_id(id: u8) -> &'static str {
    match id {
        133 => "Immolator",
        198 => "Champions Robe",
        109 => "Channeling Runes",
        186 => "Merciless Witchblade",
        176 => "Cloak of Shadows",
        197 => "Warp Throw",
        187 => "Witchblade of Kurnous",
        178 => "Providence",
        144 => "Heart of Darkness",
        196 => "Faolchu's Wing",
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
