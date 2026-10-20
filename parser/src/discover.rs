//! Locating the game's `Elite.module` without `--module`, through Steam.
//!
//! Candidates are tried in preference order:
//!
//! 1. `DOW2_INSTALL`, when set (non-Steam copies; the same variable the tests use);
//! 2. Steam apps by id, Retribution (56400) first, then the Anniversary Edition (15620);
//! 3. a top-level scan of every library's `steamapps\common\<dir>\Elite.module`, which also
//!    covers standalone Elite Mod installs and layouts the app ids do not predict.
//!
//! Every candidate must contain `Elite.module` directly. The app-id pass only sees installed
//! apps: steamlocate reads `appmanifest_*.acf`, which Steam writes when an app installs.

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use steamlocate::SteamDir;

use parser_lib::module::resolve_path;

const MODULE_FILE: &str = "Elite.module";

/// DOW2 Steam app ids, ordered by preference. Elite Mod extends Retribution (56400); the
/// Anniversary Edition (Classic, 15620) bundles Retribution as well.
const DOW2_APP_IDS: &[u32] = &[56400, 15620];

/// Install-dir override for non-Steam copies, honored before Steam.
const ENV_INSTALL: &str = "DOW2_INSTALL";

/// A found `Elite.module`. Lower `rank` wins; app-id candidates rank by position in
/// [`DOW2_APP_IDS`], filename-scan candidates rank last.
struct Candidate {
    rank: usize,
    module: PathBuf,
}

/// Finds a module file for display-name resolution. Returns a user-facing message when
/// nothing is found; the caller decides how to proceed (warn, parse without names).
pub fn locate_elite_module() -> Result<PathBuf, String> {
    if let Some(path) = from_env() {
        return Ok(path);
    }

    let steam = SteamDir::locate().map_err(|e| {
        format!(
            "could not locate Steam ({}); pass --module \"<install>/Elite.module\"",
            e
        )
    })?;
    let libraries = steam.library_paths().map_err(|e| {
        format!(
            "could not read the Steam libraries ({}); pass --module \"<install>/Elite.module\"",
            e
        )
    })?;

    let mut candidates = Vec::new();

    // App-id pass across all libraries at once.
    for (i, app_id) in DOW2_APP_IDS.iter().enumerate() {
        if let Ok(Some((app, library))) = steam.find_app(*app_id) {
            let install = library.resolve_app_dir(&app);
            if let Some(module) = resolve_path(&install, MODULE_FILE) {
                candidates.push(Candidate { rank: i, module });
            }
        }
    }

    // Filename pass over each library.
    for library in &libraries {
        if let Some(module) = scan_library(library) {
            candidates.push(Candidate {
                rank: DOW2_APP_IDS.len(),
                module,
            });
        }
    }

    pick(candidates, &libraries)
}

/// `<DOW2_INSTALL>/Elite.module`, when the variable is set and the file exists. Otherwise
/// None, so discovery falls through to Steam.
fn from_env() -> Option<PathBuf> {
    let dir = PathBuf::from(env::var_os(ENV_INSTALL)?);
    resolve_path(&dir, MODULE_FILE)
}

/// Scans one library's `steamapps\common` for any subdirectory containing `Elite.module`
/// (matched case-insensitively). Nested directories are not searched, keeping this cheap.
fn scan_library(library: &Path) -> Option<PathBuf> {
    let common = library.join("steamapps").join("common");
    for entry in fs::read_dir(common).ok()?.filter_map(Result::ok) {
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_dir() {
            continue;
        }
        if let Some(module) = resolve_path(&entry.path(), MODULE_FILE) {
            return Some(module);
        }
    }
    None
}

/// Chooses one candidate: deduplicates paths, sorts by rank (stable, so library order breaks
/// ties), warns when several remain, and reports what was searched when none did.
fn pick(mut candidates: Vec<Candidate>, searched: &[PathBuf]) -> Result<PathBuf, String> {
    let mut seen = HashSet::new();
    candidates.retain(|c| seen.insert(c.module.to_string_lossy().to_ascii_lowercase()));
    if candidates.is_empty() {
        let searched: Vec<String> = searched
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        return Err(format!(
            "no Elite.module found in the Steam libraries [{}] (searched steamapps\\common); pass --module \"<install>/Elite.module\"",
            searched.join(", ")
        ));
    }
    candidates.sort_by_key(|c| c.rank);
    let best = candidates[0].module.clone();
    if candidates.len() > 1 {
        let list: Vec<String> = candidates
            .iter()
            .map(|c| c.module.display().to_string())
            .collect();
        eprintln!(
            "warning: multiple Elite.module candidates found; using {}: {}",
            best.display(),
            list.join(", ")
        );
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("parser_discover_{}_{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn scan_library_finds_module_case_insensitively() {
        let root = temp_dir("scan_ok");
        let install = root
            .join("steamapps")
            .join("common")
            .join("Dawn of War II Retribution");
        fs::create_dir_all(&install).unwrap();
        fs::write(install.join("elite.MODULE"), b"").unwrap();
        let found = scan_library(&root).unwrap();
        assert!(
            found
                .file_name()
                .unwrap()
                .eq_ignore_ascii_case(MODULE_FILE),
            "{}",
            found.display()
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn scan_library_skips_files_and_nested() {
        let root = temp_dir("scan_skip");
        let common = root.join("steamapps").join("common");
        fs::create_dir_all(common.join("nested").join("deep")).unwrap();
        fs::create_dir_all(common.join("plain")).unwrap();
        fs::write(common.join("nested").join("deep").join("Elite.module"), b"").unwrap();
        fs::write(common.join("not_a_dir.txt"), b"").unwrap();
        assert_eq!(scan_library(&root), None);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn pick_prefers_lower_rank_over_order() {
        let a = PathBuf::from("/a/Elite.module");
        let b = PathBuf::from("/b/Elite.module");
        let candidates = vec![
            Candidate {
                rank: 1,
                module: a,
            },
            Candidate {
                rank: 0,
                module: b.clone(),
            },
        ];
        assert_eq!(pick(candidates, &[]).unwrap(), b);
    }

    #[test]
    fn pick_dedups_paths_keeping_lowest_rank() {
        let a = PathBuf::from("/a/Elite.module");
        let dup_a = PathBuf::from("/A/elite.module");
        let b = PathBuf::from("/b/Elite.module");
        let candidates = vec![
            Candidate {
                rank: 2,
                module: a,
            },
            Candidate {
                rank: 0,
                module: b.clone(),
            },
            Candidate {
                rank: 1,
                module: dup_a,
            },
        ];
        assert_eq!(pick(candidates, &[]).unwrap(), b);
    }

    #[test]
    fn pick_reports_searched_paths_when_empty() {
        let err = pick(Vec::new(), &[PathBuf::from("/libs")]).unwrap_err();
        assert!(err.contains("/libs"), "{}", err);
        assert!(err.contains("steamapps\\common"), "{}", err);
        assert!(err.contains("--module"), "{}", err);
    }

    #[test]
    #[ignore]
    fn real_locate() {
        let found = locate_elite_module().unwrap();
        assert!(
            found.file_name().unwrap().eq_ignore_ascii_case(MODULE_FILE),
            "{}",
            found.display()
        );
    }
}
