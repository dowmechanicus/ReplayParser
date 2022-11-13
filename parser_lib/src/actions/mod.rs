use serde::{ser::SerializeStruct, Serialize};

type ActionData<'a> = (&'a Vec<u8>, u32);

#[derive(Debug)]
pub struct Action {
    pub player: String,
    pub relic_id: u64,
    pub tick: u32,
    pub data: Vec<u8>,
}

impl<'a> From<ActionData<'a>> for Action {
    fn from(action_data: ActionData<'a>) -> Self {
        let (data, tick) = action_data;

        Self {
            player: String::new(),
            relic_id: 0,
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
        let data = if *(&self.data.len()) > 20 as usize {
            &self.data[1..20]
        } else {
            &self.data[1..]
        };
        state.serialize_field("relic_id", &self.relic_id)?;
        state.serialize_field("name", &self.player)?;
        state.serialize_field("tick", &self.tick)?;
        state.serialize_field("data", &data)?;
        state.end()
    }
}
