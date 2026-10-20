use color_eyre::Result;
use std::path::Path;

extern crate byteorder;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate crypto;

pub mod actions;
pub mod attrib;
pub mod blueprints;
pub mod chunky;
pub mod message;
pub mod metadata;
pub mod module;
pub mod names;
mod parse;
pub mod pipeline;
pub mod rbf;
pub mod replay;
pub mod seats;
pub mod sga;
pub mod stats;
pub mod ucs;
use actions::Action;
use parse::*;
pub use pipeline::{Chain, Context, GameData, HandlerError, ReplayHandler};
use std::collections::{BTreeSet, HashMap};

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

/// Blueprint namespace referenced by a command's leading `u32`, by (opcode, custom tag, payload
/// length). Only forms verified against current-build replays are listed. Deliberately left
/// unresolved: 13 (`CMD_Ability`, unverified), 90 and 4/16/63/64 (docs-only), 5/51 (queue
/// ordinals), 71 (stance), tag-1 forms of 53/85, 94 and 96 (not blueprints).
pub fn blueprint_kind_for(
    opcode: u8,
    tag: u8,
    length: Option<u16>,
) -> Option<blueprints::BlueprintKind> {
    use blueprints::BlueprintKind::*;
    let length = length?;
    match (opcode, tag) {
        (3, 5) if length == 4 => Some(Sbp),
        (15, 5) | (50, 5) if length == 4 => Some(Upgrade),
        (49, 5) if length == 4 => Some(Ebp),
        (78, 15) | (89, 16) if length == 35 => Some(Ebp),
        (53, 25..=29) | (85, 25..=29) if length >= 5 => Some(Ability),
        _ => None,
    }
}

/// Fills `Command.blueprint` on every command whose (opcode, tag, length) is a verified
/// blueprint form. Indices are build-specific: a replay from a different build than the
/// catalogues yields wrong names without warning. Always sets `replay.blueprints`.
pub fn resolve_blueprints(
    replay: &mut replay::ReplayInfo,
    catalogues: &blueprints::BlueprintCatalogues,
    module: &str,
) {
    let counts = catalogues.counts();
    replay.blueprints = Some(replay::BlueprintSource {
        module: module.to_string(),
        ebp_count: counts.ebp,
        sbp_count: counts.sbp,
        upgrade_count: counts.upgrade,
        ability_count: counts.ability,
    });
    let mut warned = BTreeSet::new();
    let mut resolve = |actions: &mut Vec<Action>| {
        for command in actions.iter_mut().filter_map(|a| a.command.as_mut()) {
            let custom = &command.custom;
            let Some(kind) = blueprint_kind_for(command.opcode.value(), custom.tag, custom.length)
            else {
                continue;
            };
            if custom.data.len() < 4 || Some(custom.data.len() as u16) != custom.length {
                continue;
            }
            let index = u32::from_le_bytes([
                custom.data[0],
                custom.data[1],
                custom.data[2],
                custom.data[3],
            ]);
            command.blueprint = catalogues.get(kind).resolve(index);
            if command.blueprint.is_none() && warned.insert((kind, index)) {
                eprintln!(
                    "warning: {} blueprint index {} is out of range",
                    kind.as_str(),
                    index
                );
            }
        }
    };
    resolve(&mut replay.commands);
    resolve(&mut replay.actions);
}

/// Fills `display_name` on every resolved blueprint, reading each distinct blueprint once. A
/// blueprint whose file can't be read or parsed keeps no name and is reported once on stderr.
pub fn resolve_display_names(replay: &mut replay::ReplayInfo, display: &names::DisplayNames) {
    let mut cache: HashMap<(blueprints::BlueprintKind, String), Option<String>> = HashMap::new();
    let mut fill = |actions: &mut Vec<Action>| {
        for bp in actions
            .iter_mut()
            .filter_map(|a| a.command.as_mut())
            .filter_map(|c| c.blueprint.as_mut())
        {
            let name = cache.entry((bp.kind, bp.path.clone())).or_insert_with(|| {
                match display.screen_name(bp.kind, &bp.path) {
                    Ok(name) => name,
                    Err(e) => {
                        eprintln!(
                            "warning: no display name for {} {}: {}",
                            bp.kind.as_str(),
                            bp.path,
                            e
                        );
                        None
                    }
                }
            });
            bp.display_name = name.clone();
        }
    };
    fill(&mut replay.commands);
    fill(&mut replay.actions);
}

pub fn parse_raw(file_path: String) -> Result<replay::ReplayInfo> {
    let path = Path::new(&file_path);
    let replay_info = parse_replay(path)?;

    Ok(replay_info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use actions::{CustomPayload, Opcode};
    use blueprints::BlueprintKind;

    fn action(opcode: u8, tag: u8, data: Vec<u8>) -> Action {
        let command = actions::Command {
            opcode: Opcode::new(opcode),
            sender_flags: 0,
            sender_slot: 0,
            process_type: 0,
            raw_context: [0; 4],
            receivers: Vec::new(),
            custom: CustomPayload {
                tag,
                length: Some(data.len() as u16),
                data,
            },
            blueprint: None,
        };
        Action::from_decoded(Vec::new(), 0, command)
    }

    fn index_action(opcode: u8, tag: u8, index: u32) -> Action {
        action(opcode, tag, index.to_le_bytes().to_vec())
    }

    fn padded(index: u32, len: usize) -> Vec<u8> {
        let mut data = index.to_le_bytes().to_vec();
        data.resize(len, 0);
        data
    }

    fn catalogues() -> blueprints::BlueprintCatalogues {
        let mut paths = Vec::new();
        for dir in ["ebps", "sbps", "upgrade", "ability"] {
            for name in ["a", "b"] {
                paths.push(format!(
                    "simulation\\attrib\\{}\\pvp\\{}_{}.rbf",
                    dir, dir, name
                ));
            }
        }
        blueprints::BlueprintCatalogues::from_paths(paths)
    }

    fn bp(a: &Action) -> Option<(BlueprintKind, String)> {
        a.command
            .as_ref()
            .unwrap()
            .blueprint
            .as_ref()
            .map(|b| (b.kind, b.name.clone()))
    }

    #[test]
    fn kind_table() {
        use BlueprintKind::*;
        assert_eq!(blueprint_kind_for(3, 5, Some(4)), Some(Sbp));
        assert_eq!(blueprint_kind_for(15, 5, Some(4)), Some(Upgrade));
        assert_eq!(blueprint_kind_for(50, 5, Some(4)), Some(Upgrade));
        assert_eq!(blueprint_kind_for(49, 5, Some(4)), Some(Ebp));
        assert_eq!(blueprint_kind_for(78, 15, Some(35)), Some(Ebp));
        assert_eq!(blueprint_kind_for(89, 16, Some(35)), Some(Ebp));
        for tag in 25..=29 {
            assert_eq!(blueprint_kind_for(53, tag, Some(5)), Some(Ability));
            assert_eq!(blueprint_kind_for(85, tag, Some(20)), Some(Ability));
        }
        // Excluded forms.
        assert_eq!(blueprint_kind_for(13, 25, Some(8)), None);
        assert_eq!(blueprint_kind_for(53, 1, Some(8)), None);
        assert_eq!(blueprint_kind_for(85, 1, Some(8)), None);
        assert_eq!(blueprint_kind_for(53, 24, Some(8)), None);
        assert_eq!(blueprint_kind_for(53, 30, Some(8)), None);
        assert_eq!(blueprint_kind_for(53, 25, Some(4)), None);
        assert_eq!(blueprint_kind_for(3, 5, Some(5)), None);
        assert_eq!(blueprint_kind_for(3, 6, Some(4)), None);
        assert_eq!(blueprint_kind_for(78, 15, Some(34)), None);
        assert_eq!(blueprint_kind_for(89, 16, Some(36)), None);
        assert_eq!(blueprint_kind_for(3, 5, None), None);
        for op in [4, 16, 63, 64, 5, 51, 71, 90, 94, 96] {
            assert_eq!(blueprint_kind_for(op, 5, Some(4)), None, "opcode {}", op);
            assert_eq!(blueprint_kind_for(op, 17, Some(35)), None, "opcode {}", op);
        }
    }

    #[test]
    fn resolves_each_form_and_leaves_rest() {
        let mut replay = replay::ReplayInfo {
            commands: vec![
                index_action(3, 5, 1),
                index_action(15, 5, 0),
                index_action(50, 5, 1),
                index_action(49, 5, 0),
                action(78, 15, padded(1, 35)),
                action(89, 16, padded(0, 35)),
                action(53, 26, padded(1, 9)),
                action(85, 29, padded(0, 21)),
                // Unresolved: excluded opcode, tag-1 ability, wrong length, out of range.
                index_action(13, 25, 1),
                action(53, 1, padded(1, 9)),
                action(78, 15, padded(1, 34)),
                index_action(3, 5, 99),
                index_action(94, 5, 1),
            ],
            actions: vec![index_action(3, 5, 0)],
            ..Default::default()
        };
        resolve_blueprints(&mut replay, &catalogues(), "Test.module");

        let got: Vec<_> = replay.commands.iter().map(bp).collect();
        let s = |k, n: &str| Some((k, n.to_string()));
        assert_eq!(got[0], s(BlueprintKind::Sbp, "sbps_b"));
        assert_eq!(got[1], s(BlueprintKind::Upgrade, "upgrade_a"));
        assert_eq!(got[2], s(BlueprintKind::Upgrade, "upgrade_b"));
        assert_eq!(got[3], s(BlueprintKind::Ebp, "ebps_a"));
        assert_eq!(got[4], s(BlueprintKind::Ebp, "ebps_b"));
        assert_eq!(got[5], s(BlueprintKind::Ebp, "ebps_a"));
        assert_eq!(got[6], s(BlueprintKind::Ability, "ability_b"));
        assert_eq!(got[7], s(BlueprintKind::Ability, "ability_a"));
        assert!(got[8..].iter().all(Option::is_none));
        assert_eq!(bp(&replay.actions[0]), s(BlueprintKind::Sbp, "sbps_a"));

        let resolved = replay.commands[0]
            .command
            .as_ref()
            .unwrap()
            .blueprint
            .clone()
            .unwrap();
        assert_eq!(resolved.path, "pvp\\sbps_b");
        assert_eq!(resolved.index, 1);
        let json = serde_json::to_value(replay.commands[0].command.as_ref().unwrap()).unwrap();
        assert_eq!(json["blueprint"]["kind"], "sbp");
        assert_eq!(json["blueprint"]["path"], "pvp\\sbps_b");
        let json = serde_json::to_value(replay.commands[8].command.as_ref().unwrap()).unwrap();
        assert!(json.get("blueprint").is_none());

        let source = replay.blueprints.unwrap();
        assert_eq!(source.module, "Test.module");
        assert_eq!(
            (
                source.ebp_count,
                source.sbp_count,
                source.upgrade_count,
                source.ability_count
            ),
            (2, 2, 2, 2)
        );
    }

    #[test]
    fn display_name_json_contract() {
        let mut data = 1u32.to_le_bytes().to_vec();
        data.resize(35, 0);
        let mut a = action(78, 15, data);
        let command = a.command.as_mut().unwrap();
        let mut bp = blueprints::ResolvedBlueprint {
            kind: blueprints::BlueprintKind::Ebp,
            index: 1,
            name: "n".to_string(),
            path: "p\\n".to_string(),
            display_name: Some("X".to_string()),
        };
        command.blueprint = Some(bp.clone());
        let json = serde_json::to_string(command).unwrap();
        assert!(json.contains("\"display_name\":\"X\""), "{}", json);

        bp.display_name = None;
        command.blueprint = Some(bp);
        let json = serde_json::to_string(command).unwrap();
        assert!(!json.contains("display_name"), "{}", json);
        assert!(!json.contains("null"), "{}", json);
    }

    #[test]
    fn corrupt_rbf_only_affects_that_blueprint() {
        use crate::attrib::AttribMount;
        use crate::rbf::{build_flb, build_rbf, FieldNames};
        use crate::sga::{Archive, Entry, Storage};
        use crate::ucs::UcsTable;

        let fields = [
            "ability_bag",
            "ui",
            "ui_info",
            "screen_name",
            "ui_ext",
            "squad_ui_ext",
            "upgrade_bag",
        ];
        let files: Vec<(&str, Vec<u8>)> = vec![
            ("simulation\\attrib\\fieldnames.flb", build_flb(&fields)),
            ("simulation\\attrib\\ebps\\bad.rbf", b"garbage".to_vec()),
            (
                "simulation\\attrib\\ebps\\good.rbf",
                build_rbf(&[(1, 0), (1, 1)], &[], &[(4, 4, 1), (2, 3, 100)], &[]),
            ),
        ];
        let mut blob = Vec::new();
        let mut entries = Vec::new();
        for (path, bytes) in &files {
            entries.push(Entry {
                path: path.to_string(),
                data_offset: blob.len() as u64,
                compressed_len: bytes.len() as u32,
                uncompressed_len: bytes.len() as u32,
                storage: Storage::Stored,
            });
            blob.extend_from_slice(bytes);
        }
        let file_path =
            std::env::temp_dir().join(format!("parser_lib_lib_{}_corrupt", std::process::id()));
        std::fs::write(&file_path, &blob).unwrap();
        let mount = AttribMount::from_parts(
            Vec::new(),
            vec![(
                std::fs::File::open(&file_path).unwrap(),
                Archive { entries },
            )],
        );
        // Sorted ebp catalogue: 0 = bad, 1 = good.
        let catalogues = blueprints::BlueprintCatalogues::from_mount(&mount);
        let flb =
            FieldNames::parse(&mount.read("simulation\\attrib\\fieldnames.flb").unwrap()).unwrap();
        let mut ucs = vec![0xFF, 0xFE];
        for u in "100\tGen\r\n".encode_utf16() {
            ucs.extend_from_slice(&u.to_le_bytes());
        }
        let display =
            names::DisplayNames::new(&mount, &flb, UcsTable::parse(&ucs).unwrap()).unwrap();

        let op78 = |index: u32| {
            let mut data = index.to_le_bytes().to_vec();
            data.resize(35, 0);
            action(78, 15, data)
        };
        let mut replay = replay::ReplayInfo {
            commands: vec![op78(1), op78(1), op78(0)],
            ..Default::default()
        };
        resolve_blueprints(&mut replay, &catalogues, "test");
        resolve_display_names(&mut replay, &display);
        let shown: Vec<Option<&str>> = replay
            .commands
            .iter()
            .map(|a| {
                a.command
                    .as_ref()
                    .unwrap()
                    .blueprint
                    .as_ref()
                    .unwrap()
                    .display_name
                    .as_deref()
            })
            .collect();
        assert_eq!(shown, vec![Some("Gen"), Some("Gen"), None]);
    }
}
