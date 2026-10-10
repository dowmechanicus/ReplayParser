//! Minimal parser for Dawn of War II `.module` files (INI-style).
//! Only `[global] Name` and `[attrib:common]` are read.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, PartialEq)]
pub struct ModuleFile {
    /// `[global] Name`, used to pick `<Name>.ucs`.
    pub name: Option<String>,
    /// Raw backslash paths relative to the install dir, ordered by `folder.NN`.
    pub attrib_folders: Vec<String>,
    /// Raw backslash paths relative to the install dir, ordered by `archive.NN`.
    pub attrib_archives: Vec<String>,
}

impl ModuleFile {
    pub fn parse(text: &str) -> ModuleFile {
        let mut folders: Vec<(u32, String)> = Vec::new();
        let mut archives: Vec<(u32, String)> = Vec::new();
        let mut name = None;
        let mut section = String::new();

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') {
                continue;
            }
            if line.starts_with('[') {
                section = line
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .trim()
                    .to_ascii_lowercase();
                continue;
            }
            let (key, value) = match line.find('=') {
                Some(i) => (line[..i].trim().to_ascii_lowercase(), line[i + 1..].trim()),
                None => continue,
            };
            if section == "global" {
                if key == "name" && !value.is_empty() {
                    name = Some(value.to_string());
                }
                continue;
            }
            if section != "attrib:common" {
                continue;
            }
            let (list, number) = if let Some(n) = key.strip_prefix("folder.") {
                (&mut folders, n)
            } else if let Some(n) = key.strip_prefix("archive.") {
                (&mut archives, n)
            } else {
                continue;
            };
            if let Ok(n) = number.parse::<u32>() {
                list.push((n, value.to_string()));
            }
        }

        folders.sort_by_key(|(n, _)| *n);
        archives.sort_by_key(|(n, _)| *n);
        ModuleFile {
            name,
            attrib_folders: folders.into_iter().map(|(_, v)| v).collect(),
            attrib_archives: archives.into_iter().map(|(_, v)| v).collect(),
        }
    }

    pub fn open(path: &Path) -> io::Result<ModuleFile> {
        let bytes = fs::read(path)?;
        Ok(ModuleFile::parse(&String::from_utf8_lossy(&bytes)))
    }
}

/// Resolves a backslash-separated path relative to `root`, matching each component
/// case-insensitively (ASCII) against the directory entries on disk. Returns None if any
/// component does not exist. An exact-case match is preferred when it exists.
pub fn resolve_path(root: &Path, rel: &str) -> Option<PathBuf> {
    let mut current = root.to_path_buf();
    for component in rel.split('\\').filter(|c| !c.is_empty()) {
        let exact = current.join(component);
        if exact.exists() {
            current = exact;
            continue;
        }
        let found = fs::read_dir(&current)
            .ok()?
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(component)
            })?;
        current = found.path();
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn parse_orders_and_filters() {
        let text = "\
[global]
archive.01 = Wrong\\Other.sga

; a comment
[Attrib:Common]
Archive.10 = B.sga

  FOLDER.02 =  Second
folder.01=First
archive.2 = A.sga
bogus = x
folder.xx = y
; archive.03 = Commented.sga

[sound:common]
archive.01 = Sound.sga
";
        let module = ModuleFile::parse(text);
        assert_eq!(module.attrib_folders, vec!["First", "Second"]);
        assert_eq!(module.attrib_archives, vec!["A.sga", "B.sga"]);
    }

    #[test]
    fn parses_global_name() {
        let m = ModuleFile::parse("[global]\nName = Elite\nUCSBaseIndex = 10000000\n[attrib:common]\narchive.01 = A.sga\n");
        assert_eq!(m.name.as_deref(), Some("Elite"));
        assert_eq!(m.attrib_archives, vec!["A.sga".to_string()]);
        assert_eq!(ModuleFile::parse("[attrib:common]\n").name, None);
    }

    #[test]
    fn resolve_path_is_case_insensitive() {
        let root = env::temp_dir().join(format!("dow2_module_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Elite").join("Archives")).unwrap();
        fs::write(
            root.join("Elite").join("Archives").join("Elite_attrib.sga"),
            b"",
        )
        .unwrap();

        let resolved = resolve_path(&root, "elite\\ARCHIVES\\\\Elite_Attrib.sga");
        let missing = resolve_path(&root, "Elite\\Nope\\Elite_attrib.sga");
        let exact = resolve_path(&root, "Elite\\Archives");
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(
            resolved,
            Some(root.join("Elite").join("Archives").join("Elite_attrib.sga"))
        );
        assert_eq!(missing, None);
        assert_eq!(exact, Some(root.join("Elite").join("Archives")));
    }

    #[test]
    #[ignore]
    fn open_real_install() {
        let install = match env::var_os("DOW2_INSTALL") {
            Some(v) => PathBuf::from(v),
            None => return,
        };
        let module = ModuleFile::open(&install.join("Elite.module")).unwrap();
        assert_eq!(
            module.attrib_archives,
            vec![
                "Elite\\Archives\\Elite_Attrib.sga",
                "Elite\\Archives\\Elite_Campaign_Attrib.sga",
                "Elite\\Archives\\GameAttrib_orig.sga",
            ]
        );
        for archive in &module.attrib_archives {
            let path = resolve_path(&install, archive).expect(archive);
            assert!(path.is_file(), "{:?}", path);
        }
        for folder in &module.attrib_folders {
            assert_eq!(resolve_path(&install, folder), None, "{}", folder);
        }
    }
}
