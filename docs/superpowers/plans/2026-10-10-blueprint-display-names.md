# Blueprint Display Names Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a readable `display_name` (e.g. "SM Power Generator", "Infiltrate") to every resolved `command.blueprint`, using the blueprint's `screen_name` UCS id and the module's localisation files.

**Architecture:** A shared `AttribMount` reads the module's `[attrib:common]` archives once, with the first source winning per path. It feeds both the existing `BlueprintCatalogues` and a new lazy lookup. For each distinct resolved blueprint, the lookup inflates the RBF from the SGA (`sga.rs`), walks it to the per-kind `screen_name` field with key ids taken from `fieldnames.flb` (`rbf.rs`), and maps the id to text through `DOW2.ucs` + `<module Name>.ucs` (`ucs.rs`). If names can't be loaded, the run warns once and omits the field; it never fails the parse.

**Tech Stack:** Rust (parser_lib edition 2018, parser edition 2021), `byteorder`, `serde`, new direct dep `miniz_oxide = "0.8"` (already in `Cargo.lock` as 0.8.9; adds no new crates).

**Spec:** `docs/superpowers/specs/2026-10-10-display-names-findings.md` (format analysis with hex-level layouts, override rule, coverage and test pins). Section numbers below (§1.2, §3.4 …) refer to it.

## Global Constraints

- Only new dependency: `miniz_oxide = "0.8"` in `parser_lib/Cargo.toml`. No `tempfile`, no `flate2`.
- `parser_lib` is edition 2018: `TryFrom`/`TryInto` need `use std::convert::{TryFrom, TryInto};`.
- Missing or broken name data (no locale folder, no `DOW2.ucs`, no `fieldnames.flb`, a corrupt RBF) must never fail the parse. Warn on stderr and leave `display_name` absent.
- `display_name` is serialized only when `Some` (`skip_serializing_if = "Option::is_none"`). `name` and `path` stay unchanged; `path` remains the identity because display names are not unique (§5).
- Key ids are resolved by **name** from `fieldnames.flb` at load time. Never hard-code `61` etc.
- No game files are checked into the repo. Unit tests build synthetic SGA/RBF/FLB/UCS bytes. Real-data tests are `#[ignore]` and read `DOW2_INSTALL` via `env::var_os("DOW2_INSTALL")`, returning early when unset (existing pattern in `blueprints.rs`).
- Every task ends green: `cargo build --all-targets`, `cargo test`, `cargo clippy --all-targets` with no new warnings in touched files. Run `rustfmt --edition 2021 <file>` only on files the task touched.
- The second binary `parser_lib/src/main.rs` re-declares modules. Any module that `blueprints.rs` (or another module that binary declares) starts depending on must be declared there with `#[allow(dead_code)]`.
- Agent per task (CLAUDE.md): Tasks 1, 3, 4 → `sonnet-impl-low`; Tasks 2, 5 → `sonnet-impl-medium`; Task 6 (docs recording findings) → `sonnet-impl-medium`. Each task is followed by `sonnet-review-medium` against this plan; `opus-review-high` reviews the whole branch at the end.

## Review Focus

1. **Non-English or partial install** (no `GameAssets\Locale\English`, or no `DOW2.ucs`): the parse succeeds, one warning is printed, and no `display_name` is emitted. The test lives in Task 5 (`load_fails_without_locale`).
2. **Future build without `fieldnames.flb`**, or with the flb missing a needed field name: same graceful outcome. The tests live in Task 5 (`load_fails_without_field_names`) and Task 5 (`new_fails_when_field_missing`).
3. **One corrupt or truncated RBF**: only that blueprint lacks a name, with one warning for it, and every other blueprint is still named. The test lives in Task 5 (`corrupt_rbf_only_affects_that_blueprint`).
4. **Stale dictionary key** (an RBF entry whose key is one past the flb end, §3.3): it must not make parsing or lookup fail. The test lives in Task 3 (`unknown_key_is_ignored`).
5. **Override**: a blueprint present in both Elite and orig archives must show the Elite text, e.g. ability 960 → "Drop Pod Sternguard Veterans", not "Drop Pod". The test lives in Task 5's real-install pins (`real_install_pins`) and Task 2 (`first_source_wins`).

---

### Task 1: SGA entry content reading

**Files:**
- Modify: `parser_lib/Cargo.toml` (add dep)
- Modify: `parser_lib/src/sga.rs` (storage mapping lines 160-164; new `Entry::read`; tests)

**Interfaces:**
- Consumes: nothing new.
- Produces:
  - `pub fn storage_from_byte(b: u8) -> Storage` (0 → `Stored`, 1 | 2 → `Zlib`, other → `Unknown(b)`).
  - `impl Entry { pub fn read<R: Read + Seek>(&self, r: &mut R) -> io::Result<Vec<u8>> }`. It returns the uncompressed bytes. Errors (`ErrorKind::InvalidData`): data out of bounds, zlib failure, length ≠ `uncompressed_len`, `Storage::Unknown`.

- [ ] **Step 1: Add the dependency**

`parser_lib/Cargo.toml` `[dependencies]`:
```toml
miniz_oxide = "0.8"
```
Run `cargo build -p parser_lib` and check that `Cargo.lock` gained no new `[[package]]` (only the dependency line for parser_lib changes).

- [ ] **Step 2: Write the failing tests** (append inside `mod tests` in `sga.rs`)

```rust
    fn zlib_entry(payload: &[u8], prefix: usize) -> (Vec<u8>, Entry) {
        let z = miniz_oxide::deflate::compress_to_vec_zlib(payload, 6);
        let mut bytes = vec![0u8; prefix];
        bytes.extend_from_slice(&z);
        let entry = Entry {
            path: "simulation\\attrib\\x.rbf".to_string(),
            data_offset: prefix as u64,
            compressed_len: z.len() as u32,
            uncompressed_len: payload.len() as u32,
            storage: Storage::Zlib,
        };
        (bytes, entry)
    }

    #[test]
    fn storage_bytes_1_and_2_are_zlib() {
        assert_eq!(storage_from_byte(0), Storage::Stored);
        assert_eq!(storage_from_byte(1), Storage::Zlib);
        assert_eq!(storage_from_byte(2), Storage::Zlib);
        assert_eq!(storage_from_byte(7), Storage::Unknown(7));
    }

    #[test]
    fn reads_zlib_entry() {
        let payload = b"RBF V0.1 payload payload payload";
        let (bytes, entry) = zlib_entry(payload, 260);
        assert_eq!(entry.read(&mut Cursor::new(bytes)).unwrap(), payload.to_vec());
    }

    #[test]
    fn reads_stored_entry() {
        let mut bytes = vec![0u8; 8];
        bytes.extend_from_slice(b"abcd");
        let entry = Entry {
            path: "a".to_string(),
            data_offset: 8,
            compressed_len: 4,
            uncompressed_len: 4,
            storage: Storage::Stored,
        };
        assert_eq!(entry.read(&mut Cursor::new(bytes)).unwrap(), b"abcd".to_vec());
    }

    #[test]
    fn read_rejects_truncated_data() {
        let (mut bytes, entry) = zlib_entry(b"hello hello hello", 4);
        bytes.truncate(bytes.len() - 1);
        let err = entry.read(&mut Cursor::new(bytes)).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }

    #[test]
    fn read_rejects_corrupt_zlib() {
        let (mut bytes, entry) = zlib_entry(b"hello hello hello", 0);
        for b in bytes.iter_mut().skip(2) {
            *b ^= 0xFF;
        }
        assert_eq!(
            entry.read(&mut Cursor::new(bytes)).unwrap_err().kind(),
            ErrorKind::InvalidData
        );
    }

    #[test]
    fn read_rejects_length_mismatch() {
        let (bytes, mut entry) = zlib_entry(b"hello hello hello", 0);
        entry.uncompressed_len += 1;
        assert_eq!(
            entry.read(&mut Cursor::new(bytes)).unwrap_err().kind(),
            ErrorKind::InvalidData
        );
    }

    #[test]
    fn read_rejects_unknown_storage() {
        let (bytes, mut entry) = zlib_entry(b"x", 0);
        entry.storage = Storage::Unknown(9);
        assert!(entry.read(&mut Cursor::new(bytes)).is_err());
    }

    #[test]
    #[ignore]
    fn real_archive_reads_sm_generator() {
        let install = match env::var_os("DOW2_INSTALL") {
            Some(v) => std::path::PathBuf::from(v),
            None => return,
        };
        let path = install.join("Elite/Archives/Elite_attrib.sga");
        let archive = Archive::open(&path).unwrap();
        let entry = archive
            .entries
            .iter()
            .find(|e| e.path.to_ascii_lowercase().ends_with("ebps\\pvp\\race_marine\\buildings\\sm_generator.rbf"))
            .unwrap();
        assert_eq!((entry.compressed_len, entry.uncompressed_len), (2521, 5913));
        assert_eq!(entry.storage, Storage::Zlib);
        let data = entry.read(&mut File::open(&path).unwrap()).unwrap();
        assert_eq!(data.len(), 5913);
        assert_eq!(&data[..8], b"RBF V0.1");
    }
```
In `parses_entries`, the fixture uses storage bytes 1, 0, 7; leave it as is (1 still maps to `Zlib`).

- [ ] **Step 2b: Run them and check they fail**

Run: `cargo test -p parser_lib --lib sga::`
Expected: compile errors, because `storage_from_byte` and `Entry::read` are not defined.

- [ ] **Step 3: Implement**

Replace the `match file_cur.read_u8()? { … }` at `sga.rs:160-164` with `let storage = storage_from_byte(file_cur.read_u8()?);`. Then add:

```rust
/// Maps the TOC storage byte. Bytes 1 and 2 are both zlib streams (`78 DA`); 2 is what every
/// `.rbf` in the Retribution attrib archives uses. 0 is assumed to mean stored (never observed).
pub fn storage_from_byte(b: u8) -> Storage {
    match b {
        0 => Storage::Stored,
        1 | 2 => Storage::Zlib,
        other => Storage::Unknown(other),
    }
}

impl Entry {
    /// Reads this entry's data from the archive stream `r` and inflates it if compressed.
    pub fn read<R: Read + Seek>(&self, r: &mut R) -> io::Result<Vec<u8>> {
        let len = r.seek(SeekFrom::End(0))?;
        let clen = u64::from(self.compressed_len);
        if self.data_offset > len || clen > len - self.data_offset {
            return Err(invalid(format!("{}: data out of bounds", self.path)));
        }
        r.seek(SeekFrom::Start(self.data_offset))?;
        let mut raw = vec![0; clen as usize];
        r.read_exact(&mut raw)?;
        let ulen = self.uncompressed_len as usize;
        let data = match self.storage {
            Storage::Stored => raw,
            Storage::Zlib => miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&raw, ulen)
                .map_err(|e| invalid(format!("{}: zlib error {:?}", self.path, e)))?,
            Storage::Unknown(b) => {
                return Err(invalid(format!("{}: unsupported storage {}", self.path, b)))
            }
        };
        if data.len() != ulen {
            return Err(invalid(format!(
                "{}: got {} bytes, expected {}",
                self.path,
                data.len(),
                ulen
            )));
        }
        Ok(data)
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p parser_lib --lib sga::` and `DOW2_INSTALL="<install>" cargo test -p parser_lib --lib sga:: -- --ignored`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add parser_lib/Cargo.toml Cargo.lock parser_lib/src/sga.rs
git commit -m "feat(sga): read and inflate archive entry data"
```

---

### Task 2: Attrib mount (first source wins) and module `Name`

**Files:**
- Create: `parser_lib/src/attrib.rs`
- Modify: `parser_lib/src/module.rs` (add `name`, parse `[global]`)
- Modify: `parser_lib/src/blueprints.rs` (`from_mount`; `from_module` delegates; move `collect_files` out)
- Modify: `parser_lib/src/lib.rs` (`pub mod attrib;`)
- Modify: `parser_lib/src/main.rs` (`#[allow(dead_code)] mod attrib;`)

**Interfaces:**
- Consumes: `Entry::read` (Task 1).
- Produces:
  - `ModuleFile { pub name: Option<String>, … }`: the `[global] Name` value (`Some("Elite")` for Elite.module).
  - `attrib::normalize(path: &str) -> String`: lowercases and drops everything before `simulation\attrib\`.
  - `attrib::AttribMount`:
    - `open(install_dir: &Path, module: &ModuleFile) -> io::Result<AttribMount>`
    - `from_parts(folders: Vec<(String, PathBuf)>, archives: Vec<(File, Archive)>) -> AttribMount`
    - `paths(&self) -> impl Iterator<Item = &str>`
    - `archive_of(&self, path: &str) -> Option<usize>`
    - `read(&self, path: &str) -> io::Result<Vec<u8>>`. A path that isn't present gives `ErrorKind::NotFound`.
  - `BlueprintCatalogues::from_mount(mount: &AttribMount) -> BlueprintCatalogues`. `from_module` keeps its signature.

- [ ] **Step 1: Write the failing tests**

In `module.rs` tests:
```rust
    #[test]
    fn parses_global_name() {
        let m = ModuleFile::parse("[global]\nName = Elite\nUCSBaseIndex = 10000000\n[attrib:common]\narchive.01 = A.sga\n");
        assert_eq!(m.name.as_deref(), Some("Elite"));
        assert_eq!(m.attrib_archives, vec!["A.sga".to_string()]);
        assert_eq!(ModuleFile::parse("[attrib:common]\n").name, None);
    }
```
Existing tests that build `ModuleFile { attrib_folders, attrib_archives }` literals need `name: None` (or `..Default::default()`).

New `attrib.rs` tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sga::{Entry, Storage};
    use std::env;

    /// Writes `bytes` to a per-process temp file and opens it.
    fn temp_file(tag: &str, bytes: &[u8]) -> (PathBuf, File) {
        let path = env::temp_dir().join(format!("parser_lib_attrib_{}_{}", std::process::id(), tag));
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
        assert_eq!(normalize("Data\\Simulation\\Attrib\\SBPS\\X.rbf"), "simulation\\attrib\\sbps\\x.rbf");
        assert_eq!(normalize("Other\\File.TXT"), "other\\file.txt");
    }

    #[test]
    fn first_source_wins() {
        let (_, a) = temp_file("a", b"AAAA");
        let (_, b) = temp_file("b", b"BBBBCCCC");
        let first = Archive { entries: vec![stored("simulation\\attrib\\x.rbf", 0, 4)] };
        let second = Archive {
            entries: vec![
                stored("simulation\\attrib\\X.rbf", 0, 4),
                stored("simulation\\attrib\\y.rbf", 4, 4),
            ],
        };
        let mount = AttribMount::from_parts(Vec::new(), vec![(a, first), (b, second)]);
        assert_eq!(mount.read("simulation\\attrib\\x.rbf").unwrap(), b"AAAA".to_vec());
        assert_eq!(mount.archive_of("SIMULATION\\ATTRIB\\X.RBF"), Some(0));
        assert_eq!(mount.read("simulation\\attrib\\y.rbf").unwrap(), b"CCCC".to_vec());
        assert_eq!(mount.archive_of("simulation\\attrib\\y.rbf"), Some(1));
        let mut paths: Vec<&str> = mount.paths().collect();
        paths.sort();
        assert_eq!(paths, vec!["simulation\\attrib\\x.rbf", "simulation\\attrib\\y.rbf"]);
        assert_eq!(
            mount.read("simulation\\attrib\\z.rbf").unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }

    #[test]
    fn loose_folder_precedes_archives() {
        let (loose, _) = temp_file("loose", b"LOOSE");
        let (_, a) = temp_file("c", b"ARCH");
        let archive = Archive { entries: vec![stored("simulation\\attrib\\x.rbf", 0, 4)] };
        let mount = AttribMount::from_parts(
            vec![("simulation\\attrib\\x.rbf".to_string(), loose)],
            vec![(a, archive)],
        );
        assert_eq!(mount.read("simulation\\attrib\\x.rbf").unwrap(), b"LOOSE".to_vec());
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
        let drop_pod = "simulation\\attrib\\ability\\pvp\\race_marine\\globals\\sm_spawn_drop_pod.rbf";
        assert_eq!(mount.archive_of(drop_pod), Some(0)); // Elite_Attrib beats GameAttrib_orig (§2)
        let flb = "simulation\\attrib\\fieldnames.flb";
        assert_eq!(mount.archive_of(flb), Some(1)); // only in Elite_Campaign_Attrib
        assert_eq!(mount.read(flb).unwrap().len(), 53210);
        let gen = mount.read("simulation\\attrib\\ebps\\pvp\\race_marine\\buildings\\sm_generator.rbf").unwrap();
        assert_eq!(gen.len(), 5913);
    }
}
```

- [ ] **Step 2: Run them and check they fail**

Run: `cargo test -p parser_lib --lib attrib:: module::`
Expected: compile errors (`attrib` module, `ModuleFile::name` missing).

- [ ] **Step 3: Implement**

`module.rs`: add `pub name: Option<String>` (doc: "`[global] Name`, used to pick `<Name>.ucs`"). Replace the `in_attrib` tracking in `parse` with a lowercased section name:
```rust
        let mut name = None;
        let mut section = String::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') {
                continue;
            }
            if line.starts_with('[') {
                section = line.trim_start_matches('[').trim_end_matches(']').trim().to_ascii_lowercase();
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
            // … existing folder./archive. handling unchanged …
        }
```
Also update the module doc comment ("Only `[global] Name` and `[attrib:common]` are read").

`attrib.rs`:
```rust
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
    pub fn from_parts(folders: Vec<(String, PathBuf)>, archives: Vec<(File, Archive)>) -> AttribMount {
        let mut index = HashMap::new();
        for (rel, full) in folders {
            index.entry(normalize(&rel)).or_insert(Source::Folder(full));
        }
        for (a, (_, archive)) in archives.iter().enumerate() {
            for (e, entry) in archive.entries.iter().enumerate() {
                index
                    .entry(normalize(&entry.path))
                    .or_insert(Source::Archive { archive: a, entry: e });
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
pub(crate) fn collect_files(dir: &Path, prefix: &str, out: &mut Vec<String>) -> io::Result<()> {
    // body moved verbatim from blueprints.rs
}
```

`blueprints.rs`: delete `collect_files` and the archive/folder loops. Remove the now-unused imports (`fs`, `resolve_path`, `Archive`), then:
```rust
    /// Reads every archive and loose attrib folder listed by `module_file` once.
    pub fn from_module(install_dir: &Path, module_file: &Path) -> io::Result<BlueprintCatalogues> {
        let module = ModuleFile::open(module_file)?;
        Ok(BlueprintCatalogues::from_mount(&AttribMount::open(install_dir, &module)?))
    }

    pub fn from_mount(mount: &AttribMount) -> BlueprintCatalogues {
        BlueprintCatalogues::from_paths(mount.paths().map(str::to_string))
    }
```
`from_paths` already deduplicates and sorts, so the order is unchanged. The existing `real_install` test (2591/715/1751/2399 plus index pins) guards that.

`lib.rs`: `pub mod attrib;`. `parser_lib/src/main.rs`: `#[allow(dead_code)] mod attrib;`.

- [ ] **Step 4: Run the tests**

Run: `cargo test` and `DOW2_INSTALL="<install>" cargo test -- --ignored`
Expected: all pass, including the unchanged `blueprints::tests::real_install`.

- [ ] **Step 5: Commit**

```bash
git add parser_lib/src/attrib.rs parser_lib/src/module.rs parser_lib/src/blueprints.rs parser_lib/src/lib.rs parser_lib/src/main.rs
git commit -m "feat(parser): shared attrib mount with first-archive-wins lookup"
```

---

### Task 3: RBF reader and field-name dictionary

**Files:**
- Create: `parser_lib/src/rbf.rs`
- Modify: `parser_lib/src/lib.rs` (`pub mod rbf;`)

**Interfaces:**
- Consumes: `AttribMount` (Task 2), in the real-install test only.
- Produces:
  - `rbf::Value { Bool(bool), Float(f32), Int(u32), Str(String), Table(u32) }` (derive `Clone, Debug, PartialEq`).
  - `rbf::FieldNames::parse(bytes: &[u8]) -> io::Result<FieldNames>`, `key(&self, name: &str) -> Option<u16>`, `name(&self, key: u16) -> Option<&str>`, `len(&self) -> usize`.
  - `rbf::Rbf::parse(bytes: &[u8]) -> io::Result<Rbf>` and `lookup(&self, keys: &[u16]) -> io::Result<Option<Value>>`.
  - Test helpers for Task 5: `#[cfg(test)] pub(crate) fn build_rbf(tables: &[(u32, u32)], index: &[u32], entries: &[(u16, u16, u32)], strings: &[u8]) -> Vec<u8>` and `#[cfg(test)] pub(crate) fn build_flb(names: &[&str]) -> Vec<u8>`.

Layout (§3.1–3.3): a 40-byte header (`RBF V0.1`, then `tables_off, table_count, index_off, index_count, entries_off, entry_count, strings_off, strings_len`, all u32). Table record `u32 count, u32 first`: count 0 is empty, count 1 means `first` is an entry index, count ≥ 2 means children are `index[first..first+count]`. Entry `u16 type, u16 key, u32 value`, with types 0 bool, 1 float, 2 int, 3 string (offset into strings: `u32 len` + bytes, Latin-1), 4 table. Table 0 is the root. Keys can repeat (first match wins), and keys may be unknown to the flb. FLB: `u32 n`, `n × u32` offsets relative to byte 4, NUL-terminated names.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
pub(crate) fn build_rbf(
    tables: &[(u32, u32)],
    index: &[u32],
    entries: &[(u16, u16, u32)],
    strings: &[u8],
) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    let tables_off = HEADER_LEN as u32;
    let index_off = tables_off + 8 * tables.len() as u32;
    let entries_off = index_off + 4 * index.len() as u32;
    let strings_off = entries_off + 8 * entries.len() as u32;
    for v in [
        tables_off,
        tables.len() as u32,
        index_off,
        index.len() as u32,
        entries_off,
        entries.len() as u32,
        strings_off,
        strings.len() as u32,
    ] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for (count, first) in tables {
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&first.to_le_bytes());
    }
    for i in index {
        out.extend_from_slice(&i.to_le_bytes());
    }
    for (ty, key, value) in entries {
        out.extend_from_slice(&ty.to_le_bytes());
        out.extend_from_slice(&key.to_le_bytes());
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.extend_from_slice(strings);
    out
}

#[cfg(test)]
pub(crate) fn build_flb(names: &[&str]) -> Vec<u8> {
    let mut offsets = Vec::new();
    let mut blob = Vec::new();
    let base = 4 * names.len() as u32; // offsets are relative to byte 4
    for name in names {
        offsets.push(base + blob.len() as u32);
        blob.extend_from_slice(name.as_bytes());
        blob.push(0);
    }
    let mut out = (names.len() as u32).to_le_bytes().to_vec();
    for o in offsets {
        out.extend_from_slice(&o.to_le_bytes());
    }
    out.extend_from_slice(&blob);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::path::PathBuf;

    /// Root → entry0 (table key 10 → t1); t1 = [entry1 int key 20 = 0, entry2 table key 11 → t2];
    /// t2 = [entry3 str key 30, entry4 int key 61 = 9040588, entry5 int key 61 = 7 (duplicate)].
    fn sample() -> Vec<u8> {
        let mut strings = 13u32.to_le_bytes().to_vec();
        strings.extend_from_slice(b"types\\ui_info");
        build_rbf(
            &[(1, 0), (2, 0), (3, 2)],
            &[1, 2, 3, 4, 5],
            &[
                (4, 10, 1),
                (2, 20, 0),
                (4, 11, 2),
                (3, 30, 0),
                (2, 61, 9040588),
                (2, 61, 7),
            ],
            &strings,
        )
    }

    #[test]
    fn looks_up_nested_value() {
        let rbf = Rbf::parse(&sample()).unwrap();
        assert_eq!(rbf.lookup(&[10, 11, 61]).unwrap(), Some(Value::Int(9040588)));
        assert_eq!(rbf.lookup(&[10, 11, 30]).unwrap(), Some(Value::Str("types\\ui_info".to_string())));
        assert_eq!(rbf.lookup(&[10, 20]).unwrap(), Some(Value::Int(0)));
    }

    #[test]
    fn missing_key_or_non_table_is_none() {
        let rbf = Rbf::parse(&sample()).unwrap();
        assert_eq!(rbf.lookup(&[10, 11, 99]).unwrap(), None);
        assert_eq!(rbf.lookup(&[10, 20, 61]).unwrap(), None);
        assert_eq!(rbf.lookup(&[]).unwrap(), None);
    }

    #[test]
    fn unknown_key_is_ignored() {
        // Key 2476 is one past the dictionary in the real flb; it is just a number here.
        let bytes = build_rbf(&[(2, 0)], &[0, 1], &[(2, 2476, 5), (2, 61, 9)], &[]);
        let rbf = Rbf::parse(&bytes).unwrap();
        assert_eq!(rbf.lookup(&[61]).unwrap(), Some(Value::Int(9)));
    }

    #[test]
    fn rejects_bad_header() {
        let mut bad_magic = sample();
        bad_magic[0] = b'X';
        assert!(Rbf::parse(&bad_magic).is_err());
        let mut short = sample();
        short.pop();
        assert!(Rbf::parse(&short).is_err());
        assert!(Rbf::parse(b"RBF V0.1").is_err());
    }

    #[test]
    fn out_of_range_references_are_errors() {
        let bytes = build_rbf(&[(2, 0)], &[0, 9], &[(2, 61, 1)], &[]);
        assert!(Rbf::parse(&bytes).unwrap().lookup(&[61]).is_err());
        let bytes = build_rbf(&[(1, 0)], &[], &[(4, 1, 5)], &[]);
        assert!(Rbf::parse(&bytes).unwrap().lookup(&[1, 2]).is_err());
        let bytes = build_rbf(&[(1, 0)], &[], &[(3, 1, 0)], &[]);
        assert!(Rbf::parse(&bytes).unwrap().lookup(&[1]).is_err());
        let bytes = build_rbf(&[(1, 0)], &[], &[(9, 1, 0)], &[]);
        assert!(Rbf::parse(&bytes).unwrap().lookup(&[1]).is_err());
    }

    #[test]
    fn field_names_round_trip() {
        let flb = FieldNames::parse(&build_flb(&["ability_bag", "ui", "screen_name"])).unwrap();
        assert_eq!(flb.len(), 3);
        assert_eq!(flb.key("screen_name"), Some(2));
        assert_eq!(flb.key("missing"), None);
        assert_eq!(flb.name(1), Some("ui"));
        assert_eq!(flb.name(3), None);
        let mut truncated = build_flb(&["a"]);
        truncated.pop(); // drop the NUL
        assert!(FieldNames::parse(&truncated).is_err());
    }

    #[test]
    #[ignore]
    fn real_install_tier_2_and_corpus() {
        use crate::attrib::AttribMount;
        use crate::blueprints::BlueprintKind;
        use crate::module::ModuleFile;
        let install = match env::var_os("DOW2_INSTALL") {
            Some(v) => PathBuf::from(v),
            None => return,
        };
        let module = ModuleFile::open(&install.join("Elite.module")).unwrap();
        let mount = AttribMount::open(&install, &module).unwrap();
        let flb = FieldNames::parse(&mount.read("simulation\\attrib\\fieldnames.flb").unwrap()).unwrap();
        assert_eq!(flb.len(), 2476);
        assert_eq!(flb.key("screen_name"), Some(61));
        let keys: Vec<u16> = ["upgrade_bag", "ui_info", "screen_name"]
            .iter()
            .map(|n| flb.key(n).unwrap())
            .collect();
        let tier_2 = mount.read("simulation\\attrib\\upgrade\\pvp\\race_marine\\research\\tier_2.rbf").unwrap();
        assert_eq!(Rbf::parse(&tier_2).unwrap().lookup(&keys).unwrap(), Some(Value::Int(9040588)));
        // Every winning blueprint of the four kinds parses.
        let mut n = 0;
        for path in mount.paths().filter(|p| p.ends_with(".rbf")) {
            if BlueprintKind::ALL.iter().any(|k| path.starts_with(k.dir())) {
                Rbf::parse(&mount.read(path).unwrap()).unwrap_or_else(|e| panic!("{}: {}", path, e));
                n += 1;
            }
        }
        assert_eq!(n, 2591 + 715 + 1751 + 2399);
    }
}
```

- [ ] **Step 2: Run them and check they fail**

Run: `cargo test -p parser_lib --lib rbf::`
Expected: compile errors (module missing).

- [ ] **Step 3: Implement** (`rbf.rs`, with `pub mod rbf;` in `lib.rs`)

```rust
//! Reader for compiled attribute files (`RBF V0.1`) and the key dictionary
//! `simulation\attrib\fieldnames.flb`. All integers are little-endian. Keys in an RBF are
//! indices into the dictionary; values are bool, float, int (UCS ids are ints), string or table.

use std::convert::TryFrom;
use std::io::{self, Error, ErrorKind};

const MAGIC: &[u8; 8] = b"RBF V0.1";
const HEADER_LEN: usize = 40;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Float(f32),
    Int(u32),
    Str(String),
    Table(u32),
}

fn invalid(msg: String) -> Error {
    Error::new(ErrorKind::InvalidData, msg)
}

fn u32_at(bytes: &[u8], at: usize) -> io::Result<u32> {
    let b = bytes
        .get(at..at.checked_add(4).ok_or_else(|| invalid("offset overflow".to_string()))?)
        .ok_or_else(|| invalid(format!("u32 at {} out of bounds", at)))?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Field names indexed by key id.
#[derive(Debug)]
pub struct FieldNames {
    names: Vec<String>,
}

impl FieldNames {
    pub fn parse(bytes: &[u8]) -> io::Result<FieldNames> {
        let n = u32_at(bytes, 0)? as usize;
        if n > bytes.len() / 4 {
            return Err(invalid(format!("field name count {} too large", n)));
        }
        let mut names = Vec::with_capacity(n);
        for i in 0..n {
            let start = 4 + u32_at(bytes, 4 + 4 * i)? as usize;
            let rest = bytes
                .get(start..)
                .ok_or_else(|| invalid(format!("field name {} out of bounds", i)))?;
            let end = rest
                .iter()
                .position(|&b| b == 0)
                .ok_or_else(|| invalid(format!("field name {} not terminated", i)))?;
            names.push(String::from_utf8_lossy(&rest[..end]).into_owned());
        }
        Ok(FieldNames { names })
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    pub fn key(&self, name: &str) -> Option<u16> {
        let i = self.names.iter().position(|n| n == name)?;
        u16::try_from(i).ok()
    }

    pub fn name(&self, key: u16) -> Option<&str> {
        self.names.get(usize::from(key)).map(String::as_str)
    }
}

/// A parsed RBF. Table 0 is the root.
#[derive(Debug)]
pub struct Rbf {
    tables: Vec<(u32, u32)>,
    index: Vec<u32>,
    entries: Vec<(u16, u16, u32)>,
    strings: Vec<u8>,
}

impl Rbf {
    /// Parses and validates the header; section layout must be contiguous and fill the file.
    pub fn parse(bytes: &[u8]) -> io::Result<Rbf> {
        if bytes.len() < HEADER_LEN || &bytes[..8] != MAGIC {
            return Err(invalid("not an RBF V0.1 file".to_string()));
        }
        let h = |i: usize| u32_at(bytes, 8 + 4 * i).map(u64::from);
        let (tables_off, table_count) = (h(0)?, h(1)?);
        let (index_off, index_count) = (h(2)?, h(3)?);
        let (entries_off, entry_count) = (h(4)?, h(5)?);
        let (strings_off, strings_len) = (h(6)?, h(7)?);
        if tables_off != HEADER_LEN as u64
            || table_count == 0
            || index_off != tables_off + 8 * table_count
            || entries_off != index_off + 4 * index_count
            || strings_off != entries_off + 8 * entry_count
            || strings_off + strings_len != bytes.len() as u64
        {
            return Err(invalid("inconsistent RBF header".to_string()));
        }
        let tables = (0..table_count as usize)
            .map(|i| {
                let at = tables_off as usize + 8 * i;
                Ok((u32_at(bytes, at)?, u32_at(bytes, at + 4)?))
            })
            .collect::<io::Result<_>>()?;
        let index = (0..index_count as usize)
            .map(|i| u32_at(bytes, index_off as usize + 4 * i))
            .collect::<io::Result<_>>()?;
        let entries = (0..entry_count as usize)
            .map(|i| {
                let at = entries_off as usize + 8 * i;
                let head = u32_at(bytes, at)?;
                Ok(((head & 0xFFFF) as u16, (head >> 16) as u16, u32_at(bytes, at + 4)?))
            })
            .collect::<io::Result<_>>()?;
        Ok(Rbf {
            tables,
            index,
            entries,
            strings: bytes[strings_off as usize..].to_vec(),
        })
    }

    /// Entry indices of a table's children (count 1 = `first` is the entry itself).
    fn children(&self, table: u32) -> io::Result<Vec<u32>> {
        let &(count, first) = self
            .tables
            .get(table as usize)
            .ok_or_else(|| invalid(format!("table {} out of range", table)))?;
        let ids: Vec<u32> = match count {
            0 => Vec::new(),
            1 => vec![first],
            n => {
                let start = first as usize;
                let end = start
                    .checked_add(n as usize)
                    .filter(|&e| e <= self.index.len())
                    .ok_or_else(|| invalid(format!("table {} index range out of bounds", table)))?;
                self.index[start..end].to_vec()
            }
        };
        if let Some(bad) = ids.iter().find(|&&e| e as usize >= self.entries.len()) {
            return Err(invalid(format!("entry {} out of range", bad)));
        }
        Ok(ids)
    }

    fn value(&self, entry: u32) -> io::Result<Value> {
        let (ty, _, v) = self.entries[entry as usize];
        Ok(match ty {
            0 => Value::Bool(v != 0),
            1 => Value::Float(f32::from_bits(v)),
            2 => Value::Int(v),
            3 => {
                let at = v as usize;
                let len = u32_at(&self.strings, at)? as usize;
                let text = self
                    .strings
                    .get(at + 4..at + 4 + len)
                    .ok_or_else(|| invalid(format!("string at {} out of bounds", at)))?;
                Value::Str(text.iter().map(|&b| b as char).collect())
            }
            4 if (v as usize) < self.tables.len() => Value::Table(v),
            4 => return Err(invalid(format!("table {} out of range", v))),
            other => return Err(invalid(format!("unknown entry type {}", other))),
        })
    }

    /// Follows `keys` from the root, taking the first child with each key. Ok(None) when a key
    /// is missing, an intermediate value is not a table, or `keys` is empty.
    pub fn lookup(&self, keys: &[u16]) -> io::Result<Option<Value>> {
        let mut table = 0u32;
        for (depth, key) in keys.iter().enumerate() {
            let found = self
                .children(table)?
                .into_iter()
                .find(|&e| self.entries[e as usize].1 == *key);
            let Some(entry) = found else {
                return Ok(None);
            };
            let value = self.value(entry)?;
            if depth + 1 == keys.len() {
                return Ok(Some(value));
            }
            match value {
                Value::Table(t) => table = t,
                _ => return Ok(None),
            }
        }
        Ok(None)
    }
}
```
Place `build_rbf`/`build_flb` (from Step 1) after the impls.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p parser_lib --lib rbf::` and `DOW2_INSTALL="<install>" cargo test -p parser_lib --lib rbf:: -- --ignored`
Expected: all pass, and the corpus test parses 7,456 files.

- [ ] **Step 5: Commit**

```bash
git add parser_lib/src/rbf.rs parser_lib/src/lib.rs
git commit -m "feat(parser): RBF attribute reader and field-name dictionary"
```

---

### Task 4: UCS localisation reader

**Files:**
- Create: `parser_lib/src/ucs.rs`
- Modify: `parser_lib/src/lib.rs` (`pub mod ucs;`)

**Interfaces:**
- Consumes: nothing.
- Produces: `ucs::UcsTable` with `parse(bytes: &[u8]) -> io::Result<UcsTable>`, `merge(&mut self, other: UcsTable)` (fill-only: existing ids are kept), `get(&self, id: u32) -> Option<&str>` (trimmed; `None` when absent or blank), `len()`, `is_empty()`. Derive `Debug, Default`.

Format (§4): BOM `FF FE`, UTF-16LE. Records are `<decimal id>\t<text>` separated by CRLF; values may contain a bare LF and further tabs. The last line may lack CRLF. Lines that don't parse are skipped.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::path::PathBuf;

    fn ucs(text: &str) -> Vec<u8> {
        let mut out = vec![0xFF, 0xFE];
        for u in text.encode_utf16() {
            out.extend_from_slice(&u.to_le_bytes());
        }
        out
    }

    #[test]
    fn parses_records() {
        let t = UcsTable::parse(&ucs(
            "1\tPower Generator\r\n2\tLevel 2 Stronghold \r\n3\tline one\nline two\r\n4\ta\tb\r\n5\t\r\nnot a record\r\n6\tlast",
        ))
        .unwrap();
        assert_eq!(t.get(1), Some("Power Generator"));
        assert_eq!(t.get(2), Some("Level 2 Stronghold"));
        assert_eq!(t.get(3), Some("line one\nline two"));
        assert_eq!(t.get(4), Some("a\tb"));
        assert_eq!(t.get(5), None);
        assert_eq!(t.get(6), Some("last"));
        assert_eq!(t.get(7), None);
        assert_eq!(t.len(), 6);
    }

    #[test]
    fn rejects_missing_bom_and_odd_length() {
        assert!(UcsTable::parse(b"1\tx").is_err());
        let mut odd = ucs("1\tx");
        odd.push(0);
        assert!(UcsTable::parse(&odd).is_err());
    }

    #[test]
    fn merge_fills_without_overriding() {
        let mut a = UcsTable::parse(&ucs("1\tA\r\n")).unwrap();
        a.merge(UcsTable::parse(&ucs("1\tX\r\n10000000\tElite\r\n")).unwrap());
        assert_eq!(a.get(1), Some("A"));
        assert_eq!(a.get(10000000), Some("Elite"));
    }

    #[test]
    #[ignore]
    fn real_install_tables() {
        let install = match env::var_os("DOW2_INSTALL") {
            Some(v) => PathBuf::from(v),
            None => return,
        };
        let dir = install.join("GameAssets/Locale/English");
        let mut t = UcsTable::parse(&std::fs::read(dir.join("DOW2.ucs")).unwrap()).unwrap();
        assert_eq!(t.len(), 71720);
        let elite = UcsTable::parse(&std::fs::read(dir.join("Elite.ucs")).unwrap()).unwrap();
        assert_eq!(elite.len(), 2142);
        t.merge(elite);
        assert_eq!(t.get(9040588), Some("Level 2 Stronghold"));
        assert_eq!(t.get(10001497), Some("Drop Pod Sternguard Veterans"));
    }
}
```

- [ ] **Step 2: Run them and check they fail**

Run: `cargo test -p parser_lib --lib ucs::`
Expected: compile errors.

- [ ] **Step 3: Implement**

```rust
//! Reader for `.ucs` localisation tables: UTF-16LE with a BOM, one `<id>\t<text>` record per
//! CRLF-terminated line. Values may contain bare LF and tabs, so only CRLF and the first tab split.

use std::collections::HashMap;
use std::io::{self, Error, ErrorKind};

#[derive(Debug, Default)]
pub struct UcsTable {
    strings: HashMap<u32, String>,
}

impl UcsTable {
    pub fn parse(bytes: &[u8]) -> io::Result<UcsTable> {
        let body = bytes
            .strip_prefix(&[0xFF, 0xFE])
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, "missing UTF-16LE BOM"))?;
        if body.len() % 2 != 0 {
            return Err(Error::new(ErrorKind::InvalidData, "odd UTF-16 byte count"));
        }
        let units: Vec<u16> = body
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let text = String::from_utf16(&units)
            .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))?;
        let mut strings = HashMap::new();
        for line in text.split("\r\n") {
            let Some((id, value)) = line.split_once('\t') else {
                continue;
            };
            let Ok(id) = id.trim().parse::<u32>() else {
                continue;
            };
            strings.insert(id, value.to_string());
        }
        Ok(UcsTable { strings })
    }

    /// Adds the records of `other` whose ids are not present yet.
    pub fn merge(&mut self, other: UcsTable) {
        for (id, text) in other.strings {
            self.strings.entry(id).or_insert(text);
        }
    }

    /// Display text for `id`, trimmed. None when the id is absent or its text is blank.
    pub fn get(&self, id: u32) -> Option<&str> {
        let text = self.strings.get(&id)?.trim();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p parser_lib --lib ucs::` and `DOW2_INSTALL="<install>" cargo test -p parser_lib --lib ucs:: -- --ignored`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add parser_lib/src/ucs.rs parser_lib/src/lib.rs
git commit -m "feat(parser): UCS localisation table reader"
```

---

### Task 5: Display-name lookup, output field and CLI wiring

**Files:**
- Create: `parser_lib/src/names.rs`
- Modify: `parser_lib/src/blueprints.rs` (`BlueprintKind::screen_name_path`, `ResolvedBlueprint::display_name`, `resolve` sets `None`)
- Modify: `parser_lib/src/lib.rs` (`pub mod names;`, `resolve_display_names`)
- Modify: `parser/src/main.rs` (open module + mount once, resolve names)

**Interfaces:**
- Consumes: `AttribMount` and `ModuleFile::name` (Task 2), `FieldNames`/`Rbf`/`Value` (Task 3), `UcsTable` (Task 4), and the test helpers `rbf::build_rbf`/`rbf::build_flb`.
- Produces:
  - `BlueprintKind::screen_name_path(self) -> &'static [&'static str]`:
    - Ebp: `["ui_ext", "screen_name"]`
    - Sbp: `["squad_ui_ext", "screen_name"]`
    - Upgrade: `["upgrade_bag", "ui_info", "screen_name"]`
    - Ability: `["ability_bag", "ui", "ui_info", "screen_name"]`
  - `ResolvedBlueprint.display_name: Option<String>`, serialized only when `Some`.
  - `names::DEFAULT_LANGUAGE: &str = "English"`.
  - `names::DisplayNames<'m>`:
    - `load(mount: &'m AttribMount, install_dir: &Path, module: &ModuleFile, language: &str) -> io::Result<Self>`
    - `new(mount: &'m AttribMount, fields: &FieldNames, ucs: UcsTable) -> io::Result<Self>`
    - `screen_name(&self, kind: BlueprintKind, path: &str) -> io::Result<Option<String>>`
  - `parser_lib::resolve_display_names(replay: &mut ReplayInfo, display: &names::DisplayNames)`.

- [ ] **Step 1: Write the failing tests** (`names.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::rbf::{build_flb, build_rbf};
    use crate::sga::{Archive, Entry, Storage};
    use std::env;
    use std::fs::{self, File};
    use std::path::PathBuf;

    const FIELDS: &[&str] = &["ability_bag", "ui", "ui_info", "screen_name", "ui_ext", "squad_ui_ext", "upgrade_bag"];

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
        let file_path = env::temp_dir().join(format!("parser_lib_names_{}_{}", std::process::id(), tag));
        fs::write(&file_path, &blob).unwrap();
        AttribMount::from_parts(Vec::new(), vec![(File::open(&file_path).unwrap(), Archive { entries })])
    }

    fn names<'m>(m: &'m AttribMount) -> DisplayNames<'m> {
        let fields = FieldNames::parse(&build_flb(FIELDS)).unwrap();
        let ucs = UcsTable::parse(&ucs_bytes("100\tSM Power Generator \r\n")).unwrap();
        DisplayNames::new(m, &fields, ucs).unwrap()
    }

    #[test]
    fn resolves_screen_name() {
        let m = mount("ok", &[
            ("simulation\\attrib\\ebps\\pvp\\gen.rbf", ebp_rbf(100)),
            ("simulation\\attrib\\ebps\\pvp\\zero.rbf", ebp_rbf(0)),
            ("simulation\\attrib\\ebps\\pvp\\unknown.rbf", ebp_rbf(555)),
            ("simulation\\attrib\\ebps\\pvp\\no_ui.rbf", build_rbf(&[(0, 0)], &[], &[], &[])),
        ]);
        let n = names(&m);
        assert_eq!(n.screen_name(BlueprintKind::Ebp, "pvp\\gen").unwrap().as_deref(), Some("SM Power Generator"));
        assert_eq!(n.screen_name(BlueprintKind::Ebp, "pvp\\zero").unwrap(), None);
        assert_eq!(n.screen_name(BlueprintKind::Ebp, "pvp\\unknown").unwrap(), None);
        assert_eq!(n.screen_name(BlueprintKind::Ebp, "pvp\\no_ui").unwrap(), None);
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
        let m = mount("noloc", &[("simulation\\attrib\\fieldnames.flb", build_flb(FIELDS))]);
        let empty = env::temp_dir().join(format!("parser_lib_names_{}_empty_install", std::process::id()));
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
            assert_eq!(n.screen_name(kind, path).unwrap().as_deref(), expected, "{:?} {} {}", kind, index, path);
        }
    }
}
```

In `lib.rs` tests, add a test of `resolve_display_names` that covers a corrupt file next to a good one (it uses the existing `action(opcode, tag, data)` helper):
```rust
    #[test]
    fn corrupt_rbf_only_affects_that_blueprint() {
        use crate::attrib::AttribMount;
        use crate::rbf::{build_flb, build_rbf, FieldNames};
        use crate::sga::{Archive, Entry, Storage};
        use crate::ucs::UcsTable;

        let fields = ["ability_bag", "ui", "ui_info", "screen_name", "ui_ext", "squad_ui_ext", "upgrade_bag"];
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
        let file_path = std::env::temp_dir().join(format!("parser_lib_lib_{}_corrupt", std::process::id()));
        std::fs::write(&file_path, &blob).unwrap();
        let mount = AttribMount::from_parts(
            Vec::new(),
            vec![(std::fs::File::open(&file_path).unwrap(), Archive { entries })],
        );
        // Sorted ebp catalogue: 0 = bad, 1 = good.
        let catalogues = blueprints::BlueprintCatalogues::from_mount(&mount);
        let flb = FieldNames::parse(&mount.read("simulation\\attrib\\fieldnames.flb").unwrap()).unwrap();
        let mut ucs = vec![0xFF, 0xFE];
        for u in "100\tGen\r\n".encode_utf16() {
            ucs.extend_from_slice(&u.to_le_bytes());
        }
        let display = names::DisplayNames::new(&mount, &flb, UcsTable::parse(&ucs).unwrap()).unwrap();

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
                a.command.as_ref().unwrap().blueprint.as_ref().unwrap().display_name.as_deref()
            })
            .collect();
        assert_eq!(shown, vec![Some("Gen"), Some("Gen"), None]);
    }
```

- [ ] **Step 2: Run them and check they fail**

Run: `cargo test -p parser_lib --lib names:: tests::corrupt`
Expected: compile errors.

- [ ] **Step 3: Implement**

`blueprints.rs`:
```rust
    /// Key path from the RBF root to the blueprint's `screen_name` UCS id.
    pub fn screen_name_path(self) -> &'static [&'static str] {
        match self {
            BlueprintKind::Ebp => &["ui_ext", "screen_name"],
            BlueprintKind::Sbp => &["squad_ui_ext", "screen_name"],
            BlueprintKind::Upgrade => &["upgrade_bag", "ui_info", "screen_name"],
            BlueprintKind::Ability => &["ability_bag", "ui", "ui_info", "screen_name"],
        }
    }
```
In `ResolvedBlueprint`, add after `path`:
```rust
    /// Localised screen name, when the module's locale data is available and the blueprint has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
```
In `BlueprintCatalogue::resolve`, set `display_name: None`.

`names.rs`:
```rust
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
        let mut ucs = UcsTable::parse(&fs::read(dow2)?)?;
        if let Some(name) = &module.name {
            if let Some(path) = resolve_path(&locale, &format!("{}.ucs", name)) {
                ucs.merge(UcsTable::parse(&fs::read(path)?)?);
            }
        }
        DisplayNames::new(mount, &fields, ucs)
    }

    pub fn new(mount: &'m AttribMount, fields: &FieldNames, ucs: UcsTable) -> io::Result<DisplayNames<'m>> {
        let keys = BlueprintKind::ALL
            .iter()
            .map(|kind| {
                kind.screen_name_path()
                    .iter()
                    .map(|name| {
                        fields.key(name).ok_or_else(|| {
                            Error::new(ErrorKind::InvalidData, format!("field {} missing from fieldnames.flb", name))
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
        let slot = BlueprintKind::ALL.iter().position(|k| *k == kind).unwrap_or(0);
        Ok(match Rbf::parse(&bytes)?.lookup(&self.keys[slot])? {
            Some(Value::Int(id)) if id != 0 => self.ucs.get(id).map(str::to_string),
            _ => None,
        })
    }
}
```

`lib.rs`: add `pub mod names;` and the resolver:
```rust
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
            let name = cache
                .entry((bp.kind, bp.path.clone()))
                .or_insert_with(|| match display.screen_name(bp.kind, &bp.path) {
                    Ok(name) => name,
                    Err(e) => {
                        eprintln!("warning: no display name for {} {}: {}", bp.kind.as_str(), bp.path, e);
                        None
                    }
                });
            bp.display_name = name.clone();
        }
    };
    fill(&mut replay.commands);
    fill(&mut replay.actions);
}
```
(Add `use std::collections::HashMap;`. `BlueprintKind` already derives `Eq + Ord`. Add `Hash` to its derive list for the `HashMap` key.)

`parser/src/main.rs`: replace the catalogue construction block with:
```rust
    let module_path = Path::new(&module);
    let install_dir = module_path.parent().unwrap_or_else(|| Path::new("."));
    let module_file = match ModuleFile::open(module_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("could not read module {}: {}", module, e);
            process::exit(1);
        }
    };
    let mount = match AttribMount::open(install_dir, &module_file) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("could not read module {}: {}", module, e);
            process::exit(1);
        }
    };
    let catalogue = BlueprintCatalogues::from_mount(&mount);
```
After `parser_lib::resolve_blueprints(&mut replay, &catalogue, &module);` add:
```rust
            match DisplayNames::load(&mount, install_dir, &module_file, DEFAULT_LANGUAGE) {
                Ok(names) => parser_lib::resolve_display_names(&mut replay, &names),
                Err(e) => eprintln!("warning: display names unavailable: {}", e),
            }
```
with imports `parser_lib::attrib::AttribMount`, `parser_lib::module::ModuleFile`, `parser_lib::names::{DisplayNames, DEFAULT_LANGUAGE}`.

- [ ] **Step 4: Run tests and the CLI check**

Run: `cargo test`, `cargo clippy --all-targets`, `DOW2_INSTALL="<install>" cargo test -- --ignored`
Expected: all pass.

CLI coverage (spec §5: 166/166 on temp.rec):
```bash
I="/home/mark/.local/share/Steam/steamapps/common/Dawn of War II - Retribution"
cargo run -q -p parser -- temp.rec --module "$I/Elite.module" > /tmp/out.json
grep -c '"blueprint"' /tmp/out.json; grep -c '"display_name"' /tmp/out.json
```
Expected: the two counts are equal (every resolved blueprint in temp.rec is named), and stderr is empty. Spot-check that op 78 index 430 shows `"display_name": "Chaos Listening Post"` or similar and op 85 index 802 shows the stormtrooper global's name.

- [ ] **Step 5: Commit**

```bash
git add parser_lib/src/names.rs parser_lib/src/blueprints.rs parser_lib/src/lib.rs parser/src/main.rs
git commit -m "feat(parser): add localised display_name to resolved blueprints"
```

---

### Task 6: Documentation

**Files:**
- Create: `docs/game-data-formats.md`
- Modify: `README.md` ("Resolving blueprint names (`--module`)" section)
- Modify: `docs/replay-format.md` (the UCS follow-up sentence in "Catalogue rule verified for build 4156674300")

This task records format findings as fact (CLAUDE.md rule 4), so its review is analysis-grade: check every claim against the spec and keep the VERIFIED/HYPOTHESIS labels.

- [ ] **Step 1: Write `docs/game-data-formats.md`** from spec §1–§4, following the house style of `docs/replay-format.md` (offset tables, an evidence column, labels):
  - SGA v5 content: header fields, the 22-byte file record, storage byte (0 stored, assumed; 1 and 2 zlib, verified), the 260-byte preheader with CRC-32, and the extraction recipe.
  - The `[attrib:common]` override rule (first archive wins, verified for archive.01 over archive.03 with the drop-pod causality evidence). The campaign archive's rank and the loose folders' rank are hypotheses.
  - RBF V0.1: header equalities, the table-record count rule, entry types, strings, duplicate keys, no inheritance (`$REF` is a schema template).
  - `fieldnames.flb`: layout, location (campaign archive only), and the stale key 2476.
  - The per-kind `screen_name` paths table, with the 0 sentinel.
  - UCS: encoding, CRLF and first-tab rules, trimming, id ranges, the DOW2 + `<Name>.ucs` choice (hypothesis), and the Elite_campaign collisions.
- [ ] **Step 2: Update README.** Add `display_name` to the `blueprint` object description and to the JSON example; regenerate the example from real `temp.rec` output. State:
  - The fallback: the field is absent when there is no name, and `name`/`path` remain.
  - The stderr warnings.
  - That names are not unique, and that reinforce (op 49) shows the model's string (spec §5).
  - A caveat: on replays from other builds, display names are as wrong as the indices, but read more convincingly.
  - A link to `docs/game-data-formats.md`.
- [ ] **Step 3: In `docs/replay-format.md`**, replace "display names are a feasible follow-up" with a link to the new doc.
- [ ] **Step 4: Verify.** The README example matches `cargo run -q -p parser -- temp.rec --module "<install>/Elite.module"` output, and `grep -rn "follow-up" README.md docs/` shows no stale display-name follow-up.
- [ ] **Step 5: Commit**

```bash
git add docs/game-data-formats.md README.md docs/replay-format.md
git commit -m "docs: document SGA content, RBF, field names and UCS formats"
```

---

## Out of scope (YAGNI)

- A `--language` CLI flag. Only English is installed; the library already takes `language`.
- Loading `Elite_campaign.ucs`. No pvp blueprint needs it (spec §4).
- `help_text`/`icon_name`, or a `screen_name_id` field in the output.
- Eager loading of all names. Lazy loading costs about 3 ms per replay (spec §6).
- A build guard. It was removed by user decision; the docs caveat covers it.
