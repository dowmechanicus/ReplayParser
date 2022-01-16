pub fn get_unit_by_id(id: u8) -> &'static str {
    match id {
        191 => "Catachan Devils",
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

pub fn get_wargear_by_id(id: u8) -> &'static str {
    match id {
        197 => "Assault Marine Sergeant",
        218 => "Scout - Shotguns",
        217 => "Scout - Sergeant",
        216 => "Scout - Elite training",
        227 => "Vanguard Veteran Squad",
        231 => "Force Commander - Iron Halo",
        240 => "Force Commander - Alacrity armor",
        252 => "Force Commander - Power Sword",
        241 => "Terminator Force Commander",
        _ => "Unknown wargear",
    }
}

pub fn get_action_type_by_id(id: u8) -> &'static str {
    match id {
        3 => "Build unit", // confirmed
        5 => "Cancel unit or wargear action",
        15 => "Tier upgrade", // confirmed
        46 => "unknown unit action (not special ability)",
        40 => "Move action",
        44 => "Move action",  // confirmed
        47 => "Upgrade unit", // confirmed (like FC -> TFC)
        48 => "Upgrade unit",
        49 => "Attack move",
        50 => "Purchase wargear",        // confirmed
        51 => "Cancel wargear purchase", //confirmed
        52 => "Attack move",             // confirmed
        53 => "Ability",                 // confirmed
        _ => "Unknown",
    }
}

#[derive(Debug)]
pub enum ActionTypes {
    BuildUnit = 3,
    CancelUnitPurchase = 5,
    TierUpgrade = 15,
    UpgradeUnit = 47,
    PurchaseWargear = 50,
    CancelWargearPurchase = 51,
}
