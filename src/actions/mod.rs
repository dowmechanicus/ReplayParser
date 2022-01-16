use serde::{ser::SerializeStruct, Serialize};

type ActionData<'a> = (&'a Vec<u8>, u32);

#[derive(Debug)]
pub struct Action {
    pub tick: u32,
    pub data: Vec<u8>,
}

impl<'a> From<ActionData<'a>> for Action {
    fn from(action_data: ActionData<'a>) -> Self {
        let (data, tick) = action_data;

        Self {
            tick,
            data: data.clone(),
        }
    }
}

impl Serialize for Action {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Action", 2)?;
        state.serialize_field("tick", &self.tick)?;
        state.serialize_field("data", serde_json::to_string(&self.data).unwrap().as_str())?;
        state.end()
    }
}

