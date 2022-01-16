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
                wargear_name: get_hero_wargear_by_item_id(data[13]).to_string(),
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

fn get_unit_wargear_by_item_id(id: u8) -> &'static str {
   match id { 
        197 => "Assault Marine Sergeant",
        216 => "Elite training",
        217 => "Sergeant",
        218 => "Shotguns",
        227 => "Vanguard Veteran Squad",
        _ => "Unknown wargear"
   }
}

fn get_hero_wargear_by_item_id(id: u8) -> &'static str {
    match id {
        198 => "Immolator",
        197 => "Merciless Witchblade",
        196 => "Witchblade of Kurnous",
        186 => "Champion's Robe",
        188 => "Providence",
        177 => "Faolchu's Wing",
        179 => "Heart of Darkness",
        176 => "Channeling Runes",
        197 => "Warp Throw",
        187 => "Cloak of Shadows",
        178 => "Warp Throw",
        144 => "Heart of Darkness",
        196 => "Faolchu's Wing",
        195 => "Heavy Gauge Death Spinner",
        190 => "Improved Warp Generator",
        181 => "Improved Targeters",
        199 => "Entangling Web",
        189 => "Enhanced Warp Jump Generator",
        182 => "Shimmer Orb",
        200 => "Powerblades",
        191 => "Phase Armor",
        180 => "Anti-Grav Grenade",
        192 => "Doombringer",
        184 => "Fortune Armor",
        175 => "Spirit Stones",
        193 => "Singing Spear",
        185 => "Rune Armor",
        173 => "Ghosthelm",
        194 => "Gravity Blade",
        183 => "Asuryan Armor",
        174 => "Runes of Reaping",
        235 => "Refractor Shield",
        242 => "Artificer Armor",
        252 => "Power Sword",
        239 => "Artificer Armor",
        231 => "Iron Halo",
        253 => "Chainsword and Storm Shield",
        240 => "Alacrity armor",
        233 => "Teleporter Pack",
        254 => "Thunder Hammer",
        241 => "Terminator Force Commander",
        232 => "Sacred Standard",
        251 => "Power Fist",
        _ => "Unknown wargear",
    }
}
