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
