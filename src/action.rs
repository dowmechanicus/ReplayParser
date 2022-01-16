use std::fmt;
use byteorder::{self, ReadBytesExt, LittleEndian, BigEndian};

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
        let mut ds = f.debug_struct("Action");
        ds
            .field("Type", &format!("{} ({})", get_action_type_by_id(self.action_type).to_string(), self.data[1]))
            .field("Ownership ID (?)", &self.data[2])
            .field("Player", &self.data[3])
            .field("Unknown", &self.data[4]);

        {
            let mut current = &self.data[5..=6];
            let v = current.read_u16::<LittleEndian>().unwrap();
            ds.field("Action count", &format!("{:?}", v + 1));
        }
            
        match self.data[7] {
            0x0 => {
                ds.field("Action source", &"Placeable".to_string());
            },
            0x10 => {
                ds.field("Action source", &"Building".to_string());
            },
            0x20 => {
                ds.field("Action source", &"Unit".to_string());
            },
            _ => {
                ds.field("Action source", &"Unknown".to_string());
            },
        }

        ds.field("Data", &serde_json::to_string(&self.data[7..]).unwrap());
        
        match self.data[1] {
            3 => {
                ds.field("Unit Purchased", &format!("{}", &get_unit_by_item_id(self.data[13])));
            },
            47..=53 => {
                ds.field("Unit", &format!("{}", &get_unit_by_item_id(self.data[13])));
            } ,
            _ => (),
        }


        ds.finish()
    }
}

/*
The action data block has the following format:
#  TYPE  LENGTH  Description
--------------------------
1  BYTE  1    Unknown. Is always 0 though.

2  BYTE  1    Action Type (confirmed)

3  BYTE  1    Not sure. It seems half an id and half an action identifier. By building generators the higher
              4 bit are 0x8 lower 4 bit: 1 means this action is executed by player with ID 1, does not always
              correspond to the order of players in the header. I think this might be the position of the player base.
              So with fixed positioning this corresponds to the player ID, that's why that only works in fixed
              games.

              Observation on lower bits: Seem to indicate player base location.

              Values that have been observed so far [u8]: 0, 1, 128, 129

4  BYTE  1    Player ID (confirmed)

5  BYTE  1    Unknown
              Values that have been observed so far [u8]: 3

6-7  BYTE  2  A counter for the actions performed by this player. (confirmed)
              Starts at 0. This means there is a limit of 65536.

              Best read as u16 LittleEndian

8  BYTE  1    0x10 = build units at HQ, tier upgrade (T2, T3), building upgrade (e.g. Turret -> Missile Turret)
              0x20 = unit upgrades, movement
              0x0  = seems related to power nodes and placeable entities (Turret, Mines)
              (seems like any action performed by HQ like building or setting rally point is 10 while every action
              performed by a unit is 20

              Values that have been observed so far [u8] (hex): 0 (0x0), 16 (0x10), 32 (0x20)

9  BYTE  1    Unknown

              Values that have been observed so far [u8]: 0

10  BYTE  1   Always changes together with 0x10 or 0x20 two bytes before. But sometimes changes between different games.
              Is this the player location/ID?

              Values that have been observed so far [u8] in 1v1: 3, 74, 195
              Values that have been observed so far [u8] additionally: 122

11-12 BYTE  2 Most likely the unit identifier. But how/where is it assigned?
              When building power nodes and generators this has been observed to be (232, 15)

13-14 BYTE  2 Always the same? It seems so.
              When building a power node this has been observed to be (35, 107)
              When building a generator this has been observed to be (35, 98)

15-19 DWORD 4 Identifier for the item (unit, upgrade, wargear)
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

  - Unit id once unit it is on the field is not the same as the unit/item id when its purchased at the HQ

*/
