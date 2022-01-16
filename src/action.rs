use std::fmt;

#[derive(Clone, Debug, Serialize)]
pub struct ActionTick {
    pub tick: u32,
    pub actions: Vec<Action>,
    pub number_of_actions: u32,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Action {
    data: Vec<u8>,
    size: u8,
    action_type: String,
}

impl fmt::Display for Action {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Action Type: {}\nData: {:?}\n", self.action_type, self.data)
  }
}

impl Action {
    pub fn append_data(&mut self, mut data: &mut Vec<u8>) {
        self.data.append(&mut data);
    }

    pub fn set_size(&mut self, size: u8) {
        self.size = size;
    }

    pub fn set_action_type(&mut self) {
      if self.data.len() > 0 {
        self.action_type = map_action_type(self.data[1]);

        match self.data[1] {
          3 => {
            // println!("Unit purchased: {:?}", self.data[11..12]);
          }
          _ => ()
        }
      } else {
        println!("Tried to set action type on an Action with not data!");
      }
    }    
}

fn map_action_type(action: u8) -> String {
  match action {
      15 => "Wargear and Tier upgrade".to_string(),
      3 => "Build unit".to_string(),
      5 => "Cancel unit or wargear action".to_string(),
      46 => "unknown unit action (not special ability)".to_string(),
      47 => "upgrade unit action".to_string(),
      40 => "move action".to_string(),
      44 => "move action".to_string(),
      48 => "upgrade unit action".to_string(),
      49 => "attack move action".to_string(),
      _ => "unknown action".to_string(),
  }
}
