use serde::{ser::SerializeStruct, Serialize};
use std::{fmt};

use self::{building::BuildingAction, global::GlobalAction, purchase_unit::PurchaseUnitAction, purchase_wargear::PurchaseWargearAction, unit::UnitAction, unknown::UnknownAction};

pub mod building;
pub mod global;
pub mod unit;
pub mod purchase_unit;
pub mod purchase_wargear;
pub mod unknown;

pub trait ParseAction: fmt::Debug {}

type ActionData<'a> = (&'a Vec<u8>, u32);

#[derive(Debug)]
pub struct Action {
    pub tick: u32,
    pub action_id: u8,
    pub name: String,
    pub player_id: u8,
    pub player_name: String,
    pub source: u8,
    pub data: Vec<u8>,
    pub context: (u8, u8),
    pub meta: Vec<u32>,
    pub details: ActionType,
}

impl<'a> From<ActionData<'a>> for Action {
    fn from(action_data: ActionData<'a>) -> Self {
        let (data, tick) = action_data;
        let action_type = ActionType::from(action_data);
        let action_name = action_type.to_string().split("(").collect::<Vec<&str>>()[0].to_string();

        Self {
            tick,
            action_id: data[1],
            name: action_name,
            player_id: data[3] - 0xE8,
            player_name: String::new(),
            source: data[7],
            data: data.clone(),
            meta: vec![],
            context: get_action_context(data).unwrap_or((0, 0)),
            details: action_type,
        }
    }
}

impl Serialize for Action {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Action", 6)?;
        state.serialize_field("tick", &self.tick)?;
        state.serialize_field("meta", serde_json::to_string(&self.meta).unwrap().as_str())?;
        state.serialize_field("timestamp", ticks2time(&self.tick).as_str())?;
        state.serialize_field("action_id", &self.action_id)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("player_id", &self.player_id)?;
        state.serialize_field("player_name", &self.player_name)?;
        state.serialize_field("source", format!("{:#X} ({})", &self.source, &self.get_source_name()).as_str())?;
        state.serialize_field("data", serde_json::to_string(&self.data).unwrap().as_str())?;
        state.serialize_field(
            "context",
            serde_json::to_string(&self.context).unwrap().as_str(),
        )?;
        state.serialize_field("details", &self.details)?;
        state.end()
    }
}

impl Action {
    fn get_source_name(&self) -> String {
        match self.source {
            0 => "Placeable Building".to_string(),
            16 => "Building".to_string(),
            32 => "Unit".to_string(),
            _ => "unknown".to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ActionType {
    AbilityOnPlaceable(BuildingAction),
    BuildUnit(PurchaseUnitAction),
    CancelUnitOrWargear(UnitAction),
    SetRallyPoint(BuildingAction),
    UpgradeBuilding(BuildingAction),
    StopMove(UnitAction),
    Move(UnitAction),
    CapturePoint(UnitAction),
    UpgradeUnit(UnitAction),
    ReinforceUnit(UnitAction),
    PurchaseWargear(PurchaseWargearAction),
    CancelWargearPurchase(UnitAction),
    AttackMove(UnitAction),
    AbilityOnUnit(UnitAction),
    Retreat(UnitAction),
    ForceMelee(UnitAction),
    ToggleStance(UnitAction),
    PlaceBuilding(BuildingAction),
    GlobalAbility(GlobalAction),
    EnterBuildingOrVehicle(UnitAction),
    ExitBuilding(UnitAction),
    ExitVehicle(UnitAction),
    Unknown(UnknownAction),
}

impl fmt::Display for ActionType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
        // or, alternatively:
        // fmt::Debug::fmt(self, f)
    }
}

impl<'a> From<ActionData<'a>> for ActionType {
    fn from(action_data: ActionData) -> Self {
        let (data, _) = action_data;
        match data[1] {
            2 => ActionType::AbilityOnPlaceable(BuildingAction::from(action_data)),
            3 => ActionType::BuildUnit(PurchaseUnitAction::from(action_data)),
            5 => ActionType::CancelUnitOrWargear(UnitAction::from(action_data)),
            9 => ActionType::Unknown(UnknownAction::from(action_data)), // source: 0x10
            11 => ActionType::SetRallyPoint(BuildingAction::from(action_data)),
            15 => ActionType::UpgradeBuilding(BuildingAction::from(action_data)),
            23 => ActionType::ExitBuilding(UnitAction::from(action_data)),
            43 => ActionType::StopMove(UnitAction::from(action_data)),
            44 => ActionType::Move(UnitAction::from(action_data)),
            47 => ActionType::CapturePoint(UnitAction::from(action_data)),
            48 => ActionType::UpgradeUnit(UnitAction::from(action_data)),
            49 => ActionType::ReinforceUnit(UnitAction::from(action_data)),
            50 => ActionType::PurchaseWargear(PurchaseWargearAction::from(action_data)),
            51 => ActionType::CancelWargearPurchase(UnitAction::from(action_data)),
            52 => ActionType::AttackMove(UnitAction::from(action_data)),
            53 => ActionType::AbilityOnUnit(UnitAction::from(action_data)),
            56 => ActionType::EnterBuildingOrVehicle(UnitAction::from(action_data)),
            58 => ActionType::ExitVehicle(UnitAction::from(action_data)),
            61 => ActionType::Retreat(UnitAction::from(action_data)),
            70 => ActionType::ForceMelee(UnitAction::from(action_data)),
            71 => ActionType::ToggleStance(UnitAction::from(action_data)),
            78 => ActionType::PlaceBuilding(BuildingAction::from(action_data)),
            85 => ActionType::GlobalAbility(GlobalAction::from(action_data)),
            89 => ActionType::Unknown(UnknownAction::from(action_data)),
            94 => ActionType::Unknown(UnknownAction::from(action_data)), // source 0x0
            96 => ActionType::Unknown(UnknownAction::from(action_data)), // source 0x0
            98 => ActionType::Unknown(UnknownAction::from(action_data)), // source 0x0
            _ => ActionType::Unknown(UnknownAction::from(action_data)),
        }
    }
}

fn get_action_context(data: &Vec<u8>) -> Result<(u8, u8), ()> {
    if data.len() > 11 {
        Ok((data[11], data[12]))
    } else {
        Err(())
    }
}

fn ticks2time(ticks: &u32) -> String {
    let total_seconds = f32::floor(*ticks as f32 / 10.0);
    let minutes = f32::floor(total_seconds / 60.0);
    let remaining_seconds = total_seconds - (minutes * 60.0);

    format!("{:02}:{:02}", minutes, remaining_seconds)
}
