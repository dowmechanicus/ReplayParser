pub fn get_action_type_by_id(id: u8) -> &'static str {
    match id {
        2 => "Ability on placeable", // confirmed - tested only on Catachan IED
        3 => "Build unit",           // confirmed
        5 => "Cancel unit or wargear action",
        11 => "Set rally point",         // confirmed
        15 => "Upgrade building",        // confirmed
        43 => "Stop Move",               // confirmed
        44 => "Move action",             // confirmed
        47 => "Capture Ressource Point", // confirmed (same for power and req)
        48 => "Upgrade unit",
        49 => "Reinforce unit",          // confirmed
        50 => "Purchase wargear",        // confirmed
        51 => "Cancel wargear purchase", // confirmed
        52 => "Attack move",             // confirmed
        53 => "Ability on unit",         // confirmed
        61 => "Retreat unit",            // confirmed
        70 => "Force Melee",             // confirmed
        71 => "Toggle Stance",           // confirmed
        78 => "Place Building",          // confirmed (can be power node, turret, etc)
        89 => "Unknown",                 // seen - originated from a placeable object
        _ => "Unknown",
    }
}
