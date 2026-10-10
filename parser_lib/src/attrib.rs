//! The `[attrib:common]` file system of a module: loose folders, then SGA archives in
//! `archive.NN` order. The first source providing a path wins (verified for Elite_Attrib over
//! GameAttrib_orig; the rank of loose folders is assumed, none exist in the analysed install).

use std::collections::HashMap;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use crate::module::{resolve_path, ModuleFile};
use crate::sga::Archive;

const ATTRIB_ROOT: &str = "simulation\\attrib\\";

enum Source {
    Folder(PathBuf),
    Archive { archive: usize, entry: usize },
}

pub struct AttribMount {
    archives: Vec<(File, Archive)>,
    index: HashMap<String, Source>,
}

/// Lowercases `path` and drops anything before `simulation\attrib\`, so loose files and
/// archive entries share one key space. Paths without that directory are kept whole.
pub fn normalize(path: &str) -> String {
    let lower = path.to_ascii_lowercase();
    match lower.find(ATTRIB_ROOT) {
        Some(i) => lower[i..].to_string(),
        None => lower,
    }
}

impl AttribMount {
    /// Opens every archive and loose folder listed by `module`. Missing archives are an error;
    /// missing folders are skipped, as the game does.
    pub fn open(install_dir: &Path, module: &ModuleFile) -> io::Result<AttribMount> {
        let mut folders = Vec::new();
        for rel in &module.attrib_folders {
            if let Some(dir) = resolve_path(install_dir, rel) {
                if dir.is_dir() {
                    let mut files = Vec::new();
                    collect_files(&dir, "", &mut files)?;
                    for file in files {
                        let full = file.split('\\').fold(dir.clone(), |p, c| p.join(c));
                        folders.push((file, full));
                    }
                }
            }
        }
        let mut archives = Vec::new();
        for rel in &module.attrib_archives {
            let path = resolve_path(install_dir, rel).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("attrib archive not found: {}", rel),
                )
            })?;
            archives.push((File::open(&path)?, Archive::open(&path)?));
        }
        Ok(AttribMount::from_parts(folders, archives))
    }

    /// Builds the mount from loose files `(relative path, file on disk)` and opened archives,
    /// both in priority order.
    pub fn from_parts(
        folders: Vec<(String, PathBuf)>,
        archives: Vec<(File, Archive)>,
    ) -> AttribMount {
        let mut index = HashMap::new();
        for (rel, full) in folders {
            index.entry(normalize(&rel)).or_insert(Source::Folder(full));
        }
        for (a, (_, archive)) in archives.iter().enumerate() {
            for (e, entry) in archive.entries.iter().enumerate() {
                index
                    .entry(normalize(&entry.path))
                    .or_insert(Source::Archive {
                        archive: a,
                        entry: e,
                    });
            }
        }
        AttribMount { archives, index }
    }

    /// Every visible path, normalised (see [`normalize`]), each once, in no particular order.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.index.keys().map(String::as_str)
    }

    /// Position (in module order) of the archive providing `path`; None for loose files and
    /// unknown paths.
    pub fn archive_of(&self, path: &str) -> Option<usize> {
        match self.index.get(&normalize(path))? {
            Source::Archive { archive, .. } => Some(*archive),
            Source::Folder(_) => None,
        }
    }

    /// Contents of the winning copy of `path`.
    pub fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        match self.index.get(&normalize(path)) {
            Some(Source::Folder(full)) => fs::read(full),
            Some(Source::Archive { archive, entry }) => {
                let (file, archive) = &self.archives[*archive];
                let mut reader: &File = file;
                archive.entries[*entry].read(&mut reader)
            }
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} is not in the attrib mount", path),
            )),
        }
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
    use crate::sga::{Entry, Storage};
    use std::env;

    /// Writes `bytes` to a per-process temp file and opens it.
    fn temp_file(tag: &str, bytes: &[u8]) -> (PathBuf, File) {
        let path =
            env::temp_dir().join(format!("parser_lib_attrib_{}_{}", std::process::id(), tag));
        fs::write(&path, bytes).unwrap();
        let file = File::open(&path).unwrap();
        (path, file)
    }

    fn stored(path: &str, offset: u64, len: u32) -> Entry {
        Entry {
            path: path.to_string(),
            data_offset: offset,
            compressed_len: len,
            uncompressed_len: len,
            storage: Storage::Stored,
        }
    }

    #[test]
    fn normalize_strips_prefix_and_lowercases() {
        assert_eq!(
            normalize("Data\\Simulation\\Attrib\\SBPS\\X.rbf"),
            "simulation\\attrib\\sbps\\x.rbf"
        );
        assert_eq!(normalize("Other\\File.TXT"), "other\\file.txt");
    }

    #[test]
    fn first_source_wins() {
        let (_, a) = temp_file("a", b"AAAA");
        let (_, b) = temp_file("b", b"BBBBCCCC");
        let first = Archive {
            entries: vec![stored("simulation\\attrib\\x.rbf", 0, 4)],
        };
        let second = Archive {
            entries: vec![
                stored("simulation\\attrib\\X.rbf", 0, 4),
                stored("simulation\\attrib\\y.rbf", 4, 4),
            ],
        };
        let mount = AttribMount::from_parts(Vec::new(), vec![(a, first), (b, second)]);
        assert_eq!(
            mount.read("simulation\\attrib\\x.rbf").unwrap(),
            b"AAAA".to_vec()
        );
        assert_eq!(mount.archive_of("SIMULATION\\ATTRIB\\X.RBF"), Some(0));
        assert_eq!(
            mount.read("simulation\\attrib\\y.rbf").unwrap(),
            b"CCCC".to_vec()
        );
        assert_eq!(mount.archive_of("simulation\\attrib\\y.rbf"), Some(1));
        let mut paths: Vec<&str> = mount.paths().collect();
        paths.sort();
        assert_eq!(
            paths,
            vec!["simulation\\attrib\\x.rbf", "simulation\\attrib\\y.rbf"]
        );
        assert_eq!(
            mount.read("simulation\\attrib\\z.rbf").unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }

    #[test]
    fn loose_folder_precedes_archives() {
        let (loose, _) = temp_file("loose", b"LOOSE");
        let (_, a) = temp_file("c", b"ARCH");
        let archive = Archive {
            entries: vec![stored("simulation\\attrib\\x.rbf", 0, 4)],
        };
        let mount = AttribMount::from_parts(
            vec![("simulation\\attrib\\x.rbf".to_string(), loose)],
            vec![(a, archive)],
        );
        assert_eq!(
            mount.read("simulation\\attrib\\x.rbf").unwrap(),
            b"LOOSE".to_vec()
        );
        assert_eq!(mount.archive_of("simulation\\attrib\\x.rbf"), None);
    }

    #[test]
    #[ignore]
    fn real_install_override_and_flb() {
        let install = match env::var_os("DOW2_INSTALL") {
            Some(v) => PathBuf::from(v),
            None => return,
        };
        let module = ModuleFile::open(&install.join("Elite.module")).unwrap();
        assert_eq!(module.name.as_deref(), Some("Elite"));
        let mount = AttribMount::open(&install, &module).unwrap();
        let drop_pod =
            "simulation\\attrib\\ability\\pvp\\race_marine\\globals\\sm_spawn_drop_pod.rbf";
        assert_eq!(mount.archive_of(drop_pod), Some(0)); // Elite_Attrib beats GameAttrib_orig (§2)
        let flb = "simulation\\attrib\\fieldnames.flb";
        assert_eq!(mount.archive_of(flb), Some(1)); // only in Elite_Campaign_Attrib
        assert_eq!(mount.read(flb).unwrap().len(), 53210);
        let gen = mount
            .read("simulation\\attrib\\ebps\\pvp\\race_marine\\buildings\\sm_generator.rbf")
            .unwrap();
        assert_eq!(gen.len(), 5913);
    }
}
