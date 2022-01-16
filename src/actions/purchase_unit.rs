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
              unit_id: data[13],
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
    5 => "Hormagaunt Brood",
    11 => "Termagant Brood",
    13 => "Warrior Brood",
    7 => "Ravener Brood",
    9 => "Spore Mines",
    14 => "Venom Brood",
    15 => "Tyrant Guard",
    12 => "Genestealer Brood",
    4 => "Zoanthrope",
    6 => "Carnifex",
    3 => "Lictor",
    10 => "Swarmlord",
    16 => "Neurothrope",
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
    110 => "Heretics",
    121 => "Chaos Space Marines",
    113 => "Havocs",
    115 => "Noise Marines",
    120 => "Raptors",
    103 => "Bloodletters",
    123 => "Chaos Dreadnought",
    114 => "Bloodcrusher",
    118 => "Plague Marines",
    125 => "Chaos Predator",
    112 => "Great Unclean One",
    124 => "Landraider Phobos",
    177 => "Inquisitorial Storm Troopers",
    178 => "Strike Squad",
    173 => "Inquistorial Operatives",
    175 => "Purgation Squad",
    169 => "Interceptor Squad",
    176 => "Purifiers",
    170 => "Terminator Librarian",
    181 => "Grey Knight Dreadnought",
    180 => "Vindicare Assassin",
    183 => "Rhino",
    179 => "Grey Knight Terminators",
    182 => "Landraider Crusader",
    184 => "Vortimer Razorback",
    192 => "Guardsmen",
    204 => "Sentinel",
    193 => "Heavy Weapons Squad",
    191 => "Catachan Devils",
    197 => "Spotters",
    198 => "Stormtroopers",
    201 => "Chimera",
    203 => "Ogryn Squad",
    196 => "Manticore",
    202 => "Leman Russ",
    199 => "Baneblade",
    194 => "Kasrkin Squad",
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
    245 => "Sluggas",
    244 => "Shoota Boys",
    241 => "Lootas",
    248 => "Stormboyz",
    243 => "Painboy",
    247 => "Stikkbommaz",
    249 => "Tankbustas",
    252 => "Deff Dread",
    254 => "Wartrukk",
    250 => "Weirdboy",
    242 => "Nob Squad",
    240 => "Kommand Squad",
    253 => "Looted Tank",
    251 => "Battlewaggon",
    239 => "Flash Gitz",
    _ => "unknown unit",
  }
}