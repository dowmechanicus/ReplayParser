//! Catalogue of squad blueprints (SBP files) visible through a module's `[attrib:common]`
//! mount. The index carried by `CMD_BuildSquad` is a position in this catalogue.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use crate::module::{resolve_path, ModuleFile};
use crate::sga::Archive;

const SBP_DIR: &str = "simulation\\attrib\\sbps\\";
const SBP_EXT: &str = ".rbf";

/// Sorted, deduplicated list of lowercased SBP paths, each starting at `simulation\attrib\sbps\`.
///
/// Sort order is an assumption: plain byte order of the lowercased path, i.e. lowercase-folded
/// order, which matches `_stricmp`. An engine that folds to uppercase instead (e.g. an ordinal
/// ignore-case compare) would order the characters `[ \ ] ^ _` and the backtick differently
/// relative to letters, which could reorder entries across directory boundaries. This has not been
/// verified against the engine.
#[derive(Debug)]
pub struct SbpCatalogue {
    paths: Vec<String>,
}

impl SbpCatalogue {
    /// Reads every archive and loose attrib folder listed by `module_file`. Missing archives are
    /// an error; missing folders are skipped, as the game does.
    pub fn from_module(install_dir: &Path, module_file: &Path) -> io::Result<SbpCatalogue> {
        let module = ModuleFile::open(module_file)?;
        let mut all: Vec<String> = Vec::new();

        for rel in &module.attrib_archives {
            let path = resolve_path(install_dir, rel).ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, format!("attrib archive not found: {}", rel))
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
        Ok(SbpCatalogue::from_paths(all))
    }

    /// Keeps `.rbf` paths under `simulation\attrib\sbps\`, lowercases them, strips any prefix
    /// before that directory, then deduplicates and sorts.
    pub fn from_paths<I: IntoIterator<Item = String>>(paths: I) -> SbpCatalogue {
        let set: BTreeSet<String> = paths
            .into_iter()
            .filter_map(|p| {
                let lower = p.to_ascii_lowercase();
                let start = lower.find(SBP_DIR)?;
                if !lower.ends_with(SBP_EXT) {
                    return None;
                }
                Some(lower[start..].to_string())
            })
            .collect();
        SbpCatalogue { paths: set.into_iter().collect() }
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    pub fn path(&self, index: u32) -> Option<&str> {
        self.paths.get(index as usize).map(String::as_str)
    }

    /// Basename of the blueprint without the `.rbf` extension.
    pub fn name(&self, index: u32) -> Option<&str> {
        let path = self.path(index)?;
        let base = path.rsplit('\\').next().unwrap_or(path);
        Some(base.strip_suffix(SBP_EXT).unwrap_or(base))
    }
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
    fn from_paths_filters_normalizes_and_sorts() {
        let catalogue = SbpCatalogue::from_paths(vec![
            "simulation\\attrib\\sbps\\race_space_marine\\SM_Tactical_Marine.rbf".to_string(),
            "Simulation\\Attrib\\SBPS\\race_chaos\\csm_cultist.RBF".to_string(),
            "simulation\\attrib\\ebps\\race_chaos\\csm_cultist.rbf".to_string(),
            "simulation\\attrib\\sbps\\race_chaos\\readme.txt".to_string(),
            "data\\simulation\\attrib\\sbps\\race_chaos\\csm_cultist.rbf".to_string(),
            "SIMULATION\\ATTRIB\\SBPS\\race_space_marine\\sm_tactical_marine.rbf".to_string(),
            "simulation\\attrib\\sbps\\race_chaos\\csm_havoc.rbf".to_string(),
        ]);
        assert_eq!(catalogue.len(), 3);
        assert!(!catalogue.is_empty());
        assert_eq!(catalogue.path(0), Some("simulation\\attrib\\sbps\\race_chaos\\csm_cultist.rbf"));
        assert_eq!(catalogue.path(2), Some("simulation\\attrib\\sbps\\race_space_marine\\sm_tactical_marine.rbf"));
        assert_eq!(catalogue.name(0), Some("csm_cultist"));
        assert_eq!(catalogue.name(1), Some("csm_havoc"));
        assert_eq!(catalogue.name(2), Some("sm_tactical_marine"));
        assert_eq!(catalogue.path(3), None);
        assert_eq!(catalogue.name(3), None);
    }

    #[test]
    #[ignore]
    fn real_install() {
        let install = match env::var_os("DOW2_INSTALL") {
            Some(v) => PathBuf::from(v),
            None => return,
        };
        let catalogue = SbpCatalogue::from_module(&install, &install.join("Elite.module")).unwrap();
        assert_eq!(catalogue.len(), 715);
        let expected = [
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
        for (index, name) in expected.iter() {
            assert_eq!(catalogue.name(*index), Some(*name), "index {}", index);
        }
    }
}
