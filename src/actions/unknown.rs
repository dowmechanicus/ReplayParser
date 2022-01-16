use super::*;

#[derive(Debug, Default, Serialize)]
pub struct UnknownAction {
    pub data: Vec<u8>,
}
impl ParseAction for UnknownAction {}
impl<'a> From<ActionData<'a>> for UnknownAction {
    fn from(action_data: ActionData) -> Self {
        let (data, _) = action_data;
        Self { data: data.clone() }
    }
}
