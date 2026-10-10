//! Catalogues of blueprints (RBF files under `simulation\attrib\`) visible through a module's
//! `[attrib:common]` mount. The indices carried by commands such as `CMD_BuildSquad` are
//! positions in these per-kind catalogues.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use crate::module::{resolve_path, ModuleFile};
use crate::sga::Archive;

const ATTRIB_DIR: &str = "simulation\\attrib\\";
const RBF_EXT: &str = ".rbf";

/// Blueprint namespace, each backed by one directory under `simulation\attrib\`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BlueprintKind {
    Ebp,
    Sbp,
    Upgrade,
    Ability,
}

impl BlueprintKind {
    pub const ALL: [BlueprintKind; 4] = [
        BlueprintKind::Ebp,
        BlueprintKind::Sbp,
        BlueprintKind::Upgrade,
        BlueprintKind::Ability,
    ];

    /// Directory prefix (relative to the module's attrib root), ending in a backslash.
    pub fn dir(self) -> &'static str {
        match self {
            BlueprintKind::Ebp => "simulation\\attrib\\ebps\\",
            BlueprintKind::Sbp => "simulation\\attrib\\sbps\\",
            BlueprintKind::Upgrade => "simulation\\attrib\\upgrade\\",
            BlueprintKind::Ability => "simulation\\attrib\\ability\\",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            BlueprintKind::Ebp => "ebp",
            BlueprintKind::Sbp => "sbp",
            BlueprintKind::Upgrade => "upgrade",
            BlueprintKind::Ability => "ability",
        }
    }
}

/// A blueprint index resolved against a catalogue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ResolvedBlueprint {
    pub kind: BlueprintKind,
    pub index: u32,
    /// Basename without `.rbf`. Not unique across folders.
    pub name: String,
    /// Path relative to the kind's directory, without `.rbf`, backslash-separated.
    pub path: String,
}

/// Sorted, deduplicated list of lowercased `.rbf` paths of one kind, each starting at
/// `simulation\attrib\<dir>\`. Index i refers to the i-th entry.
///
/// Sort order is an assumption: plain byte order of the lowercased path including the `.rbf`
/// extension, i.e. lowercase-folded order, which matches `_stricmp`. Verified against replay data
/// for the installed build (an uppercase fold orders ability 906 wrongly).
#[derive(Debug)]
pub struct BlueprintCatalogue {
    kind: BlueprintKind,
    paths: Vec<String>,
}

impl BlueprintCatalogue {
    pub fn kind(&self) -> BlueprintKind {
        self.kind
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Path relative to the kind's directory, without `.rbf`, e.g.
    /// `pvp\race_marine\buildings\sm_generator`.
    pub fn path(&self, index: u32) -> Option<&str> {
        let full = self.paths.get(index as usize)?;
        let rel = &full[self.kind.dir().len()..];
        Some(rel.strip_suffix(RBF_EXT).unwrap_or(rel))
    }

    /// Basename of the blueprint without the `.rbf` extension.
    pub fn name(&self, index: u32) -> Option<&str> {
        let path = self.path(index)?;
        Some(path.rsplit('\\').next().unwrap_or(path))
    }

    pub fn resolve(&self, index: u32) -> Option<ResolvedBlueprint> {
        Some(ResolvedBlueprint {
            kind: self.kind,
            index,
            name: self.name(index)?.to_string(),
            path: self.path(index)?.to_string(),
        })
    }
}

/// The four blueprint catalogues of a module.
#[derive(Debug)]
pub struct BlueprintCatalogues {
    pub ebp: BlueprintCatalogue,
    pub sbp: BlueprintCatalogue,
    pub upgrade: BlueprintCatalogue,
    pub ability: BlueprintCatalogue,
}

impl BlueprintCatalogues {
    /// Reads every archive and loose attrib folder listed by `module_file` once. Missing
    /// archives are an error; missing folders are skipped, as the game does.
    pub fn from_module(install_dir: &Path, module_file: &Path) -> io::Result<BlueprintCatalogues> {
        let module = ModuleFile::open(module_file)?;
        let mut all: Vec<String> = Vec::new();

        for rel in &module.attrib_archives {
            let path = resolve_path(install_dir, rel).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("attrib archive not found: {}", rel),
                )
            })?;
            let archive = Archive::open(&path)?;
            all.extend(archive.entries.into_iter().map(|e| e.path));
        }
        for rel in &module.attrib_folders {
            if let Some(dir) = resolve_path(install_dir, rel) {
                if dir.is_dir() {
                    collect_files(&dir, "", &mut all)?;
                }
            }
        }
        Ok(BlueprintCatalogues::from_paths(all))
    }

    /// Keeps `.rbf` paths under each kind's directory, lowercases them, strips any prefix before
    /// that directory, then deduplicates and sorts per kind.
    pub fn from_paths<I: IntoIterator<Item = String>>(paths: I) -> BlueprintCatalogues {
        let mut sets: [BTreeSet<String>; 4] = Default::default();
        for p in paths {
            let lower = p.to_ascii_lowercase();
            if !lower.ends_with(RBF_EXT) || !lower.contains(ATTRIB_DIR) {
                continue;
            }
            for (i, kind) in BlueprintKind::ALL.iter().enumerate() {
                if let Some(start) = lower.find(kind.dir()) {
                    sets[i].insert(lower[start..].to_string());
                    break;
                }
            }
        }
        let [ebp, sbp, upgrade, ability] = sets;
        let make = |kind, set: BTreeSet<String>| BlueprintCatalogue {
            kind,
            paths: set.into_iter().collect(),
        };
        BlueprintCatalogues {
            ebp: make(BlueprintKind::Ebp, ebp),
            sbp: make(BlueprintKind::Sbp, sbp),
            upgrade: make(BlueprintKind::Upgrade, upgrade),
            ability: make(BlueprintKind::Ability, ability),
        }
    }

    pub fn get(&self, kind: BlueprintKind) -> &BlueprintCatalogue {
        match kind {
            BlueprintKind::Ebp => &self.ebp,
            BlueprintKind::Sbp => &self.sbp,
            BlueprintKind::Upgrade => &self.upgrade,
            BlueprintKind::Ability => &self.ability,
        }
    }

    pub fn counts(&self) -> BlueprintCounts {
        BlueprintCounts {
            ebp: self.ebp.len(),
            sbp: self.sbp.len(),
            upgrade: self.upgrade.len(),
            ability: self.ability.len(),
        }
    }
}

/// Number of entries in each catalogue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct BlueprintCounts {
    pub ebp: usize,
    pub sbp: usize,
    pub upgrade: usize,
    pub ability: usize,
}

/// Recursively appends the files under `dir` as backslash-joined paths relative to the walk root.
fn collect_files(dir: &Path, prefix: &str, out: &mut Vec<String>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = format!("{}{}", prefix, name);
        if entry.file_type()?.is_dir() {
            collect_files(&entry.path(), &format!("{}\\", rel), out)?;
        } else {
            out.push(rel);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::path::PathBuf;

    #[test]
    fn from_paths_filters_normalizes_and_sorts_per_kind() {
        let c = BlueprintCatalogues::from_paths(vec![
            "simulation\\attrib\\sbps\\race_space_marine\\SM_Tactical_Marine.rbf".to_string(),
            "Simulation\\Attrib\\SBPS\\race_chaos\\csm_cultist.RBF".to_string(),
            "simulation\\attrib\\ebps\\race_chaos\\csm_cultist.rbf".to_string(),
            "simulation\\attrib\\sbps\\race_chaos\\readme.txt".to_string(),
            "data\\simulation\\attrib\\sbps\\race_chaos\\csm_cultist.rbf".to_string(),
            "SIMULATION\\ATTRIB\\SBPS\\race_space_marine\\sm_tactical_marine.rbf".to_string(),
            "simulation\\attrib\\sbps\\race_chaos\\csm_havoc.rbf".to_string(),
            "simulation\\attrib\\upgrade\\pvp\\tier_2.rbf".to_string(),
            "simulation\\attrib\\ability\\pvp\\globals\\a.rbf".to_string(),
            "simulation\\attrib\\weapons\\x.rbf".to_string(),
        ]);
        assert_eq!(
            c.counts(),
            BlueprintCounts {
                ebp: 1,
                sbp: 3,
                upgrade: 1,
                ability: 1
            }
        );
        assert!(!c.sbp.is_empty());
        assert_eq!(c.sbp.kind(), BlueprintKind::Sbp);
        assert_eq!(c.sbp.path(0), Some("race_chaos\\csm_cultist"));
        assert_eq!(c.sbp.path(2), Some("race_space_marine\\sm_tactical_marine"));
        assert_eq!(c.sbp.name(1), Some("csm_havoc"));
        assert_eq!(c.sbp.name(3), None);
        assert_eq!(c.sbp.path(3), None);
        assert_eq!(c.ebp.name(0), Some("csm_cultist"));
        assert_eq!(c.get(BlueprintKind::Upgrade).path(0), Some("pvp\\tier_2"));
        assert_eq!(c.ability.path(0), Some("pvp\\globals\\a"));
    }

    #[test]
    fn basename_collision_and_extension_sort() {
        let c = BlueprintCatalogues::from_paths(vec![
            "simulation\\attrib\\upgrade\\pvp\\race_marine\\research\\tier_2.rbf".to_string(),
            "simulation\\attrib\\upgrade\\pvp\\race_chaos\\research\\tier_2.rbf".to_string(),
            "simulation\\attrib\\upgrade\\a.rbf".to_string(),
            "simulation\\attrib\\upgrade\\a-b.rbf".to_string(),
        ]);
        let u = &c.upgrade;
        // Sorted with the extension: "a-b.rbf" < "a.rbf".
        assert_eq!(u.path(0), Some("a-b"));
        assert_eq!(u.path(1), Some("a"));
        assert_eq!(u.name(2), Some("tier_2"));
        assert_eq!(u.name(3), Some("tier_2"));
        assert_eq!(u.path(2), Some("pvp\\race_chaos\\research\\tier_2"));
        assert_eq!(u.path(3), Some("pvp\\race_marine\\research\\tier_2"));
        let r = u.resolve(3).unwrap();
        assert_eq!(r.kind, BlueprintKind::Upgrade);
        assert_eq!(r.index, 3);
        assert!(u.resolve(4).is_none());
    }

    #[test]
    fn kind_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&BlueprintKind::Ebp).unwrap(),
            "\"ebp\""
        );
        assert_eq!(
            serde_json::to_string(&BlueprintKind::Ability).unwrap(),
            "\"ability\""
        );
    }

    #[test]
    #[ignore]
    fn real_install() {
        let install = match env::var_os("DOW2_INSTALL") {
            Some(v) => PathBuf::from(v),
            None => return,
        };
        let c = BlueprintCatalogues::from_module(&install, &install.join("Elite.module")).unwrap();
        assert_eq!(
            c.counts(),
            BlueprintCounts {
                ebp: 2591,
                sbp: 715,
                upgrade: 1751,
                ability: 2399
            }
        );
        let sbp = [
            (111, "csm_cultist"),
            (115, "csm_havoc"),
            (117, "csm_noise_marines"),
            (120, "csm_plague_marines"),
            (123, "csm_tactical_marine"),
            (129, "csm_landraider"),
            (130, "csm_predator"),
            (204, "ig_devils"),
            (205, "ig_guardsmen_squad"),
            (212, "ig_spotter_squad"),
            (224, "ig_leman_russ"),
            (226, "ig_sentinel"),
            (245, "sm_assault_marine"),
            (246, "sm_devastator"),
            (247, "sm_devastator_plasma_cannon"),
            (249, "sm_librarian"),
            (250, "sm_scout_marine"),
            (252, "sm_tactical_marine"),
            (265, "sm_predator"),
            (268, "sm_whirlwind"),
        ];
        let ebp = [
            (425, "csm_generator"),
            (661, "sm_generator"),
            (670, "sm_listening_post"),
            (577, "ig_deployable_cover"),
            (452, "csm_cultist"),
        ];
        let upgrade = [(475, "tier_2"), (50, "tier_2"), (104, "acc_sorceror_rift")];
        let ability = [
            (962, "sm_spawn_terminators"),
            (906, "ig_stormtrooper_squad_infiltrate"),
            (919, "sm_heal_target_apoth"),
        ];
        for (cat, pins) in [
            (&c.sbp, &sbp[..]),
            (&c.ebp, &ebp[..]),
            (&c.upgrade, &upgrade[..]),
            (&c.ability, &ability[..]),
        ] {
            for (index, name) in pins {
                assert_eq!(
                    cat.name(*index),
                    Some(*name),
                    "{:?} index {}",
                    cat.kind(),
                    index
                );
            }
        }
        assert_eq!(
            c.ebp.path(661),
            Some("pvp\\race_marine\\buildings\\sm_generator")
        );
        assert_eq!(
            c.ability.path(906),
            Some("pvp\\race_imperial_guard\\stormtrooper_squad\\ig_stormtrooper_squad_infiltrate")
        );
    }
}
