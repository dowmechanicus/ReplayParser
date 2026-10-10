//! Readable blueprint names: the blueprint's `screen_name` UCS id (an int in its RBF) looked up
//! in `DOW2.ucs` plus `<module Name>.ucs` from `GameAssets\Locale\<language>\`.

use std::fs;
use std::io::{self, Error, ErrorKind};
use std::path::Path;

use crate::attrib::AttribMount;
use crate::blueprints::BlueprintKind;
use crate::module::{resolve_path, ModuleFile};
use crate::rbf::{FieldNames, Rbf, Value};
use crate::ucs::UcsTable;

pub const DEFAULT_LANGUAGE: &str = "English";
const FIELD_NAMES: &str = "simulation\\attrib\\fieldnames.flb";

pub struct DisplayNames<'m> {
    mount: &'m AttribMount,
    /// Key ids of `screen_name_path()`, indexed like `BlueprintKind::ALL`.
    keys: Vec<Vec<u16>>,
    ucs: UcsTable,
}

fn not_found(msg: String) -> Error {
    Error::new(ErrorKind::NotFound, msg)
}

/// Reads and parses a UCS file; errors name the file.
fn read_ucs(path: &Path) -> io::Result<UcsTable> {
    fs::read(path)
        .and_then(|bytes| UcsTable::parse(&bytes))
        .map_err(|e| Error::new(e.kind(), format!("{}: {}", path.display(), e)))
}

impl<'m> DisplayNames<'m> {
    pub fn load(
        mount: &'m AttribMount,
        install_dir: &Path,
        module: &ModuleFile,
        language: &str,
    ) -> io::Result<DisplayNames<'m>> {
        let fields = FieldNames::parse(&mount.read(FIELD_NAMES)?)?;
        let locale = resolve_path(install_dir, &format!("GameAssets\\Locale\\{}", language))
            .ok_or_else(|| not_found(format!("locale folder for {} not found", language)))?;
        let dow2 = resolve_path(&locale, "DOW2.ucs")
            .ok_or_else(|| not_found(format!("DOW2.ucs not found in {}", locale.display())))?;
        let mut ucs = read_ucs(&dow2)?;
        if let Some(name) = &module.name {
            if let Some(path) = resolve_path(&locale, &format!("{}.ucs", name)) {
                ucs.merge(read_ucs(&path)?);
            }
        }
        DisplayNames::new(mount, &fields, ucs)
    }

    pub fn new(
        mount: &'m AttribMount,
        fields: &FieldNames,
        ucs: UcsTable,
    ) -> io::Result<DisplayNames<'m>> {
        let keys = BlueprintKind::ALL
            .iter()
            .map(|kind| {
                kind.screen_name_path()
                    .iter()
                    .map(|name| {
                        fields.key(name).ok_or_else(|| {
                            Error::new(
                                ErrorKind::InvalidData,
                                format!("field {} missing from fieldnames.flb", name),
                            )
                        })
                    })
                    .collect::<io::Result<Vec<u16>>>()
            })
            .collect::<io::Result<_>>()?;
        Ok(DisplayNames { mount, keys, ucs })
    }

    /// Screen name of the blueprint at `path` (relative to the kind's directory, no `.rbf`).
    /// Ok(None) when it has no screen name field, the id is 0, or the id is not localised.
    /// Err when the file is missing or malformed.
    pub fn screen_name(&self, kind: BlueprintKind, path: &str) -> io::Result<Option<String>> {
        let bytes = self.mount.read(&format!("{}{}.rbf", kind.dir(), path))?;
        let slot = BlueprintKind::ALL
            .iter()
            .position(|k| *k == kind)
            .unwrap_or(0);
        Ok(match Rbf::parse(&bytes)?.lookup(&self.keys[slot])? {
            Some(Value::Int(id)) if id != 0 => self.ucs.get(id).map(str::to_string),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rbf::{build_flb, build_rbf};
    use crate::sga::{Archive, Entry, Storage};
    use std::env;
    use std::fs::{self, File};
    use std::path::PathBuf;

    const FIELDS: &[&str] = &[
        "ability_bag",
        "ui",
        "ui_info",
        "screen_name",
        "ui_ext",
        "squad_ui_ext",
        "upgrade_bag",
    ];

    /// RBF whose root has `ui_ext` (key 4) → table with `screen_name` (key 3) = `id`.
    fn ebp_rbf(id: u32) -> Vec<u8> {
        build_rbf(&[(1, 0), (1, 1)], &[], &[(4, 4, 1), (2, 3, id)], &[])
    }

    fn ucs_bytes(text: &str) -> Vec<u8> {
        let mut out = vec![0xFF, 0xFE];
        for u in text.encode_utf16() {
            out.extend_from_slice(&u.to_le_bytes());
        }
        out
    }

    /// One stored-archive mount from (path, bytes) pairs, written to a temp file.
    fn mount(tag: &str, files: &[(&str, Vec<u8>)]) -> AttribMount {
        let mut blob = Vec::new();
        let mut entries = Vec::new();
        for (path, bytes) in files {
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
            env::temp_dir().join(format!("parser_lib_names_{}_{}", std::process::id(), tag));
        fs::write(&file_path, &blob).unwrap();
        AttribMount::from_parts(
            Vec::new(),
            vec![(File::open(&file_path).unwrap(), Archive { entries })],
        )
    }

    fn names<'m>(m: &'m AttribMount) -> DisplayNames<'m> {
        let fields = FieldNames::parse(&build_flb(FIELDS)).unwrap();
        let ucs = UcsTable::parse(&ucs_bytes("100\tSM Power Generator \r\n")).unwrap();
        DisplayNames::new(m, &fields, ucs).unwrap()
    }

    #[test]
    fn resolves_screen_name() {
        let m = mount(
            "ok",
            &[
                ("simulation\\attrib\\ebps\\pvp\\gen.rbf", ebp_rbf(100)),
                ("simulation\\attrib\\ebps\\pvp\\zero.rbf", ebp_rbf(0)),
                ("simulation\\attrib\\ebps\\pvp\\unknown.rbf", ebp_rbf(555)),
                (
                    "simulation\\attrib\\ebps\\pvp\\no_ui.rbf",
                    build_rbf(&[(0, 0)], &[], &[], &[]),
                ),
            ],
        );
        let n = names(&m);
        assert_eq!(
            n.screen_name(BlueprintKind::Ebp, "pvp\\gen")
                .unwrap()
                .as_deref(),
            Some("SM Power Generator")
        );
        assert_eq!(
            n.screen_name(BlueprintKind::Ebp, "pvp\\zero").unwrap(),
            None
        );
        assert_eq!(
            n.screen_name(BlueprintKind::Ebp, "pvp\\unknown").unwrap(),
            None
        );
        assert_eq!(
            n.screen_name(BlueprintKind::Ebp, "pvp\\no_ui").unwrap(),
            None
        );
        assert!(n.screen_name(BlueprintKind::Ebp, "pvp\\absent").is_err());
    }

    #[test]
    fn new_fails_when_field_missing() {
        let m = mount("nofield", &[]);
        let fields = FieldNames::parse(&build_flb(&["ui_ext"])).unwrap();
        assert!(DisplayNames::new(&m, &fields, UcsTable::default()).is_err());
    }

    #[test]
    fn load_fails_without_field_names() {
        let m = mount("noflb", &[]);
        let module = ModuleFile::default();
        assert!(DisplayNames::load(&m, &env::temp_dir(), &module, DEFAULT_LANGUAGE).is_err());
    }

    #[test]
    fn load_fails_without_locale() {
        let m = mount(
            "noloc",
            &[("simulation\\attrib\\fieldnames.flb", build_flb(FIELDS))],
        );
        let empty = env::temp_dir().join(format!(
            "parser_lib_names_{}_empty_install",
            std::process::id()
        ));
        fs::create_dir_all(&empty).unwrap();
        assert!(DisplayNames::load(&m, &empty, &ModuleFile::default(), DEFAULT_LANGUAGE).is_err());
    }

    #[test]
    #[ignore]
    fn real_install_pins() {
        use crate::blueprints::BlueprintCatalogues;
        let install = match env::var_os("DOW2_INSTALL") {
            Some(v) => PathBuf::from(v),
            None => return,
        };
        let module = ModuleFile::open(&install.join("Elite.module")).unwrap();
        let mount = AttribMount::open(&install, &module).unwrap();
        let cats = BlueprintCatalogues::from_mount(&mount);
        let n = DisplayNames::load(&mount, &install, &module, DEFAULT_LANGUAGE).unwrap();
        use BlueprintKind::*;
        let pins: &[(BlueprintKind, u32, Option<&str>)] = &[
            (Ebp, 661, Some("SM Power Generator")),
            (Ebp, 425, Some("Chaos Power Generator")),
            (Ebp, 607, Some("Lord Commissar (Hero)")),
            (Sbp, 252, Some("Tactical Marine Squad")),
            (Sbp, 129, Some("Land Raider Phobos")),
            (Sbp, 147, Some("Guardian Squad")),
            (Upgrade, 475, Some("Level 2 Stronghold")),
            (Upgrade, 50, Some("Level 2 Chaos Temple")),
            (Upgrade, 382, Some("Vanquisher")),
            (Ability, 906, Some("Infiltrate")),
            (Ability, 962, Some("Deep Strike Terminator Squad")),
            (Ability, 960, Some("Drop Pod Sternguard Veterans")),
            (Ability, 777, Some("Ol' Unreliable")),
            (Ebp, 441, None),
            (Upgrade, 49, None),
            (Ability, 1326, None),
        ];
        for &(kind, index, expected) in pins {
            let path = cats.get(kind).path(index).unwrap();
            assert_eq!(
                n.screen_name(kind, path).unwrap().as_deref(),
                expected,
                "{:?} {} {}",
                kind,
                index,
                path
            );
        }
    }
}
