use std::fmt;

use identifiers::*;

#[derive(Clone, Default, Serialize)]
pub struct Action {
    data: Vec<u8>,
    pub action_type: u8,
    pub player_id: u8,
    pub tick: u32,
}

impl fmt::Debug for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Action")
            .field("Type", &get_action_type_by_id(self.action_type).to_string())
            .field("Data", &self.data)
            .finish()
    }
}

/*
The action data block has the following format:
TYPE  LENGTH  Description
--------------------------
BYTE  1       Unknown. Is always 0 though.

BYTE  1       Action Type (confirmed)

BYTE  1       Not sure. It seems half an id and half an action identifier. By building generators the higher
              4 bit are 0x8 lower 4 bit: 1 means this action is executed by player with ID 1, does not always
              correspond to the order of players in the header. I think this might be the position of the player base.
              So with fixed positioning this corresponds to the player ID, that's why that only works in fixed
              games.
              Values that have been observed so far [u8]: 0, 1, 129

BYTE  1       Player ID (confirmed)

BYTE  1       Unknown
              Values that have been observed so far [u8]: 3

BYTE  2       A counter for the actions performed by this player. (confirmed)
              Starts at 0. This means there is a limit of 65536.

BYTE  1       0x10 = build units
              0x20 = unit upgrades, movement
              (seems like any action performed by HQ like building or setting rally point is 10 while every action
              performed by a unit is 20

              Values that have been observed so far [u8] (hex): 16 (0x10), 32 (0x20)

BYTE  1       Unknown

              Values that have been observed so far [u8]: 0

BYTE  1       Always changes together with 0x10 or 0x20 two bytes before. But sometimes changes between different games.
              Is this the player location/ID?

              Values that have been observed so far [u8]: 74, 195

BYTE  2       Most likely the unit identifier. But how/where is it assigned?

BYTE  2       Always the same? It seems so.

DWORD 4       Identifier for the item (unit, upgrade, wargear)
              See the item codes file for these values

              Identifier for the canceled unit, upgrade, wargear
              These are numbered in the order they are queued.
              Units and Base Tiers share the same numbering.
              Wargear is on a different ordering.
              Upgrades have a different ordering for every unit. These actions are numbered: upgrades (0x30) and unknown (0x2E)
 */

impl Action {
    pub fn append_data(&mut self, mut data: &mut Vec<u8>) {
        self.data.append(&mut data);
        self.set_action_type();
        self.set_player_id();
    }

    fn set_action_type(&mut self) {
        if self.data.len() > 0 {
            self.action_type = self.data[1];
        } else {
            println!("Tried to set action type on an Action with no data!");
        }
    }

    fn set_player_id(&mut self) {
        self.player_id = self.data[3];
    }
}

/*
  General observations:

  - Abilities that can be toggled are identical apart from the action counter. As far as the game is concerned you have
    used the same ability twice.

  - Targettable abilities seem to also have location information stored in them (probably origin and target coordinates)

  - Action id 47 seems related to adding unit members or transforming a unit ASM -> ASM + Sergeant or FC -> TFC

*/
