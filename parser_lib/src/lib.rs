use color_eyre::Result;
use std::path::Path;

extern crate byteorder;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate crypto;

pub mod actions;
pub mod blueprints;
pub mod chunky;
pub mod message;
pub mod metadata;
pub mod module;
mod parse;
pub mod replay;
pub mod sga;
use actions::{Action, Opcode};
use parse::*;
use std::collections::BTreeSet;

pub fn parse_file(file_path: String) -> Option<String> {
    let path = Path::new(&file_path);
    let replay = match parse_replay(path) {
        Ok(r) => Some(r),
        Err(e) => {
            println!("error: {}", e);
            None
        }
    };
    let json = serde_json::to_string_pretty(&replay).unwrap();
    Some(json)
}

/// Serializes a parsed replay as pretty-printed JSON.
pub fn to_json(replay: &replay::ReplayInfo) -> serde_json::Result<String> {
    serde_json::to_string_pretty(replay)
}

/// Fills `squad_blueprint` on every `CMD_BuildSquad` carrying a 4-byte
/// squad index (custom tag 5). Warnings go to stderr; stdout carries the JSON.
pub fn resolve_squad_names(
    replay: &mut replay::ReplayInfo,
    catalogue: &blueprints::SbpCatalogue,
    module: &str,
) {
    let mut warned = BTreeSet::new();
    let mut resolve = |actions: &mut Vec<Action>| {
        for command in actions.iter_mut().filter_map(|a| a.command.as_mut()) {
            if !matches!(command.opcode, Opcode::CMD_BuildSquad(_)) {
                continue;
            }
            let custom = &command.custom;
            if custom.tag != 5 || custom.length != Some(4) || custom.data.len() != 4 {
                continue;
            }
            let index = u32::from_le_bytes([
                custom.data[0],
                custom.data[1],
                custom.data[2],
                custom.data[3],
            ]);
            command.squad_blueprint = catalogue.name(index).map(String::from);
            if command.squad_blueprint.is_none() && warned.insert(index) {
                eprintln!("warning: squad blueprint index {} is out of range", index);
            }
        }
    };
    resolve(&mut replay.commands);
    resolve(&mut replay.actions);
    replay.blueprints = Some(replay::BlueprintSource {
        module: module.to_string(),
        sbp_count: catalogue.len(),
    });
}

pub fn parse_raw(file_path: String) -> Result<replay::ReplayInfo> {
    let path = Path::new(&file_path);
    let replay_info = parse_replay(path)?;

    Ok(replay_info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use actions::CustomPayload;

    fn build_action(opcode: u8, index: u32) -> Action {
        let command = actions::Command {
            opcode: Opcode::new(opcode),
            sender_flags: 0,
            sender_slot: 0,
            process_type: 0,
            raw_context: [0; 4],
            receivers: Vec::new(),
            custom: CustomPayload {
                tag: 5,
                length: Some(4),
                data: index.to_le_bytes().to_vec(),
            },
            squad_blueprint: None,
        };
        Action::from_decoded(Vec::new(), 0, command)
    }

    #[test]
    fn resolves_only_build_squad_with_in_range_index() {
        let catalogue = blueprints::SbpCatalogue::from_paths(vec![
            "simulation\\attrib\\sbps\\race_chaos\\csm_cultist.rbf".to_string(),
            "simulation\\attrib\\sbps\\race_chaos\\csm_havoc.rbf".to_string(),
        ]);
        let mut replay = replay::ReplayInfo::default();
        replay.commands = vec![
            build_action(3, 1),
            build_action(3, 99),
            build_action(94, 1),
        ];
        resolve_squad_names(&mut replay, &catalogue, "Test.module");

        let names: Vec<_> = replay
            .commands
            .iter()
            .map(|a| a.command.as_ref().unwrap().squad_blueprint.clone())
            .collect();
        assert_eq!(names, vec![Some("csm_havoc".to_string()), None, None]);
        let source = replay.blueprints.unwrap();
        assert_eq!(source.module, "Test.module");
        assert_eq!(source.sbp_count, 2);
    }
}
