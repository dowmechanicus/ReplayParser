//! Per-player counts of orders issued, taken from the decoded command stream.
//!
//! These count orders, not outcomes: a `SCMD_Capture` is an order to capture, not a captured point.

use crate::actions::Action;
use crate::chunky::Chunk;
use crate::replay::ReplayInfo;
use std::collections::BTreeMap;

/// `CMD_BuildSquad`.
const OP_BUILD_SQUAD: u8 = 3;
/// Queue cancel (`CMD_*`, opcode 5).
const OP_CANCEL: u8 = 5;
/// `SCMD_Capture`, covering both requisition and power points.
const OP_CAPTURE: u8 = 47;
/// `SCMD_ReinforceUnit`.
const OP_REINFORCE: u8 = 49;
/// Queue cancel (opcode 51), counted together with [`OP_CANCEL`].
const OP_CANCEL_ALT: u8 = 51;
/// `SCMD_Retreat`.
const OP_RETREAT: u8 = 61;
/// `PCMD_ResurrectHero`.
const OP_RESURRECT_HERO: u8 = 96;

#[derive(Debug, Serialize, PartialEq)]
pub struct PlayerStats {
    pub slot: u8,
    pub name: Option<String>,
    pub relic_id: Option<u64>,
    pub race: Option<String>,
    /// Every decoded command, including opcodes with no known name.
    pub actions: u32,
    pub squads_built: u32,
    pub retreats: u32,
    pub reinforcements: u32,
    pub hero_resurrections: u32,
    pub point_captures: u32,
    pub cancels: u32,
}

impl PlayerStats {
    fn new(slot: u8, name: Option<String>, relic_id: Option<u64>, race: Option<String>) -> Self {
        PlayerStats {
            slot,
            name,
            relic_id,
            race,
            actions: 0,
            squads_built: 0,
            retreats: 0,
            reinforcements: 0,
            hero_resurrections: 0,
            point_captures: 0,
            cancels: 0,
        }
    }

    fn count(&mut self, opcode: u8) {
        self.actions += 1;
        match opcode {
            OP_BUILD_SQUAD => self.squads_built += 1,
            OP_RETREAT => self.retreats += 1,
            OP_REINFORCE => self.reinforcements += 1,
            OP_RESURRECT_HERO => self.hero_resurrections += 1,
            OP_CAPTURE => self.point_captures += 1,
            OP_CANCEL | OP_CANCEL_ALT => self.cancels += 1,
            _ => {}
        }
    }
}

/// One entry per seated player, sorted by slot, plus one nameless entry for each command sender
/// slot that matches no player. Uses `commands` only, never the legacy filtered `actions`.
pub fn compute(replay: &ReplayInfo) -> Vec<PlayerStats> {
    let mut stats: BTreeMap<u8, PlayerStats> = BTreeMap::new();
    for chunk in &replay.players {
        if let Chunk::Player(p) = chunk {
            if let Some(slot) = p.slot {
                stats.insert(
                    slot,
                    PlayerStats::new(
                        slot,
                        Some(p.name.clone()),
                        Some(p.relic_id),
                        Some(p.race.clone()),
                    ),
                );
            }
        }
    }
    for command in replay
        .commands
        .iter()
        .filter_map(|a: &Action| a.command.as_ref())
    {
        stats
            .entry(command.sender_slot)
            .or_insert_with(|| PlayerStats::new(command.sender_slot, None, None, None))
            .count(command.opcode.value());
    }
    stats.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::{self, CustomPayload, Opcode};
    use crate::chunky::Player;

    fn action(opcode: u8, slot: u8) -> Action {
        let command = actions::Command {
            opcode: Opcode::new(opcode),
            sender_flags: 0,
            sender_slot: slot,
            process_type: 0,
            raw_context: [0; 4],
            receivers: Vec::new(),
            custom: CustomPayload {
                tag: 0xff,
                length: None,
                data: Vec::new(),
            },
            blueprint: None,
        };
        Action::from_decoded(Vec::new(), 0, command)
    }

    fn player(slot: u8, name: &str) -> Chunk {
        Chunk::Player(Player {
            name: name.to_string(),
            race: "space_marines".to_string(),
            relic_id: 100 + slot as u64,
            slot: Some(slot),
            ..Default::default()
        })
    }

    fn replay(players: Vec<Chunk>, commands: Vec<Action>) -> ReplayInfo {
        ReplayInfo {
            players,
            commands,
            ..Default::default()
        }
    }

    #[test]
    fn each_opcode_increments_only_its_own_field() {
        // Expected [squads_built, retreats, reinforcements, hero_resurrections,
        // point_captures, cancels] after a single command of each opcode.
        let cases: [(u8, [u32; 6]); 7] = [
            (3, [1, 0, 0, 0, 0, 0]),
            (61, [0, 1, 0, 0, 0, 0]),
            (49, [0, 0, 1, 0, 0, 0]),
            (96, [0, 0, 0, 1, 0, 0]),
            (47, [0, 0, 0, 0, 1, 0]),
            (5, [0, 0, 0, 0, 0, 1]),
            (51, [0, 0, 0, 0, 0, 1]),
        ];
        for (opcode, expected) in cases {
            let r = replay(vec![player(0, "a")], vec![action(opcode, 0)]);
            let s = &compute(&r)[0];
            assert_eq!(s.actions, 1, "opcode {}", opcode);
            let got = [
                s.squads_built,
                s.retreats,
                s.reinforcements,
                s.hero_resurrections,
                s.point_captures,
                s.cancels,
            ];
            assert_eq!(got, expected, "opcode {}", opcode);
        }
    }

    #[test]
    fn actions_counts_unknown_opcodes() {
        let r = replay(
            vec![player(0, "a")],
            vec![action(97, 0), action(3, 0), action(200, 0)],
        );
        let s = &compute(&r)[0];
        assert_eq!(s.actions, 3);
        assert_eq!(s.squads_built, 1);
    }

    #[test]
    fn cancels_sum_opcodes_5_and_51() {
        let r = replay(
            vec![player(0, "a")],
            vec![action(5, 0), action(51, 0), action(51, 0)],
        );
        assert_eq!(compute(&r)[0].cancels, 3);
    }

    #[test]
    fn players_are_separated_by_slot_and_sorted() {
        let r = replay(
            vec![player(1, "b"), player(0, "a")],
            vec![action(3, 1), action(3, 0), action(61, 0), action(3, 1)],
        );
        let s = compute(&r);
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].slot, s[0].name.as_deref()), (0, Some("a")));
        assert_eq!((s[0].squads_built, s[0].retreats, s[0].actions), (1, 1, 2));
        assert_eq!(s[0].relic_id, Some(100));
        assert_eq!(s[0].race.as_deref(), Some("space_marines"));
        assert_eq!((s[1].slot, s[1].name.as_deref()), (1, Some("b")));
        assert_eq!((s[1].squads_built, s[1].retreats, s[1].actions), (2, 0, 2));
    }

    #[test]
    fn unknown_slot_gets_extra_entry() {
        let r = replay(vec![player(0, "a")], vec![action(3, 5)]);
        let s = compute(&r);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].actions, 0);
        assert_eq!(s[1].slot, 5);
        assert_eq!(
            (s[1].name.clone(), s[1].relic_id, s[1].race.clone()),
            (None, None, None)
        );
        assert_eq!(s[1].squads_built, 1);
    }

    #[test]
    fn action_without_command_is_skipped() {
        let mut a = action(3, 0);
        a.command = None;
        let r = replay(vec![player(0, "a")], vec![a]);
        assert_eq!(compute(&r)[0].actions, 0);
    }

    #[test]
    fn empty_replay_cases() {
        let r = replay(vec![player(0, "a"), player(1, "b")], Vec::new());
        let s = compute(&r);
        assert_eq!(s.len(), 2);
        assert!(s.iter().all(|p| p.actions == 0 && p.cancels == 0));
        assert!(compute(&replay(Vec::new(), Vec::new())).is_empty());
    }

    #[test]
    fn unseated_player_gets_no_entry() {
        let unseated = Chunk::Player(Player {
            name: "ghost".to_string(),
            slot: None,
            ..Default::default()
        });
        let r = replay(vec![player(0, "a"), unseated], vec![action(3, 0)]);
        let s = compute(&r);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].name.as_deref(), Some("a"));
        assert_eq!(s[0].squads_built, 1);
    }

    #[test]
    fn json_has_expected_fields() {
        let r = replay(vec![player(0, "a")], vec![action(3, 0)]);
        let json = serde_json::to_value(compute(&r)).unwrap();
        assert_eq!(
            json,
            serde_json::json!([{
                "slot": 0,
                "name": "a",
                "relic_id": 100,
                "race": "space_marines",
                "actions": 1,
                "squads_built": 1,
                "retreats": 0,
                "reinforcements": 0,
                "hero_resurrections": 0,
                "point_captures": 0,
                "cancels": 0,
            }])
        );
    }
}
