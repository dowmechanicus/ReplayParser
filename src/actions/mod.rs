use std::fmt;

use self::{building::BuildingAction, unit::UnitAction, unknown::UnknownAction};

pub mod building;
pub mod unit;
pub mod unknown;

pub trait ParseAction: fmt::Debug {}

type ActionData<'a> = (&'a Vec<u8>, u32);

fn get_player_id(data: &Vec<u8>) -> u8 {
    data[3] - 0xE8
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ActionType {
    AbilityOnPlaceable(BuildingAction),
    BuildUnit(UnitAction),
    CancelUnitOrWargear(UnitAction),
    SetRallyPoint(BuildingAction),
    UpgradeBuilding(BuildingAction),
    StopMove(UnitAction),
    Move(UnitAction),
    CapturePoint(UnitAction),
    UpgradeUnit(UnitAction),
    ReinforceUnit(UnitAction),
    PurchaseWargear(UnitAction),
    CancelWargearPurchase(UnitAction),
    AttackMove(UnitAction),
    AbilityOnUnit(UnitAction),
    Retreat(UnitAction),
    ForceMelee(UnitAction),
    ToggleStance(UnitAction),
    PlaceBuilding(BuildingAction),
    Unknown(UnknownAction),
}

impl<'a> From<ActionData<'a>> for ActionType {
    fn from(action_data: ActionData) -> Self {
        let (data, _) = action_data;
        match data[1] {
            2 => ActionType::AbilityOnPlaceable(BuildingAction::from(action_data)),
            3 => ActionType::BuildUnit(UnitAction::from(action_data)),
            5 => ActionType::CancelUnitOrWargear(UnitAction::from(action_data)),
            11 => ActionType::SetRallyPoint(BuildingAction::from(action_data)),
            15 => ActionType::UpgradeBuilding(BuildingAction::from(action_data)),
            43 => ActionType::StopMove(UnitAction::from(action_data)),
            44 => ActionType::Move(UnitAction::from(action_data)),
            47 => ActionType::CapturePoint(UnitAction::from(action_data)),
            48 => ActionType::UpgradeUnit(UnitAction::from(action_data)),
            49 => ActionType::ReinforceUnit(UnitAction::from(action_data)),
            50 => ActionType::PurchaseWargear(UnitAction::from(action_data)),
            51 => ActionType::CancelWargearPurchase(UnitAction::from(action_data)),
            52 => ActionType::AttackMove(UnitAction::from(action_data)),
            53 => ActionType::AbilityOnUnit(UnitAction::from(action_data)),
            61 => ActionType::Retreat(UnitAction::from(action_data)),
            70 => ActionType::ForceMelee(UnitAction::from(action_data)),
            71 => ActionType::ToggleStance(UnitAction::from(action_data)),
            78 => ActionType::PlaceBuilding(BuildingAction::from(action_data)),
            89 => ActionType::Unknown(UnknownAction::from(action_data)),
            _ => ActionType::Unknown(UnknownAction::from(action_data)),
        }
    }
}
