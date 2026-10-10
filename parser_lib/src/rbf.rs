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
        .get(
            at..at
                .checked_add(4)
                .ok_or_else(|| invalid("offset overflow".to_string()))?,
        )
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
        // Offsets may overlap, so resolve each one to its terminator by binary search over the
        // NUL positions (linear memory and O(n log n) time) and bound the total copied bytes.
        let nuls: Vec<usize> = bytes
            .iter()
            .enumerate()
            .filter(|(_, &b)| b == 0)
            .map(|(i, _)| i)
            .collect();
        let mut names = Vec::with_capacity(n);
        let mut total = 0usize;
        for i in 0..n {
            let start = 4usize
                .checked_add(u32_at(bytes, 4 + 4 * i)? as usize)
                .filter(|&s| s <= bytes.len())
                .ok_or_else(|| invalid(format!("field name {} out of bounds", i)))?;
            let at = nuls.partition_point(|&p| p < start);
            let end = *nuls
                .get(at)
                .ok_or_else(|| invalid(format!("field name {} not terminated", i)))?;
            total += end - start;
            if total > bytes.len() {
                return Err(invalid("field names overlap".to_string()));
            }
            names.push(String::from_utf8_lossy(&bytes[start..end]).into_owned());
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
                Ok((
                    (head & 0xFFFF) as u16,
                    (head >> 16) as u16,
                    u32_at(bytes, at + 4)?,
                ))
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

    #[test]
    fn field_names_reject_overlapping_offsets() {
        // 20000 offsets all pointing at one 40000 byte name.
        let n = 20_000usize;
        let mut b = (n as u32).to_le_bytes().to_vec();
        for _ in 0..n {
            b.extend_from_slice(&(4 * n as u32).to_le_bytes());
        }
        b.resize(b.len() + 40_000, b'a');
        b.push(0);
        let err = FieldNames::parse(&b).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }

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
        assert_eq!(
            rbf.lookup(&[10, 11, 61]).unwrap(),
            Some(Value::Int(9040588))
        );
        assert_eq!(
            rbf.lookup(&[10, 11, 30]).unwrap(),
            Some(Value::Str("types\\ui_info".to_string()))
        );
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
        let flb =
            FieldNames::parse(&mount.read("simulation\\attrib\\fieldnames.flb").unwrap()).unwrap();
        assert_eq!(flb.len(), 2476);
        assert_eq!(flb.key("screen_name"), Some(61));
        let keys: Vec<u16> = ["upgrade_bag", "ui_info", "screen_name"]
            .iter()
            .map(|n| flb.key(n).unwrap())
            .collect();
        let tier_2 = mount
            .read("simulation\\attrib\\upgrade\\pvp\\race_marine\\research\\tier_2.rbf")
            .unwrap();
        assert_eq!(
            Rbf::parse(&tier_2).unwrap().lookup(&keys).unwrap(),
            Some(Value::Int(9040588))
        );
        // Every winning blueprint of the four kinds parses.
        let mut n = 0;
        for path in mount.paths().filter(|p| p.ends_with(".rbf")) {
            if BlueprintKind::ALL.iter().any(|k| path.starts_with(k.dir())) {
                Rbf::parse(&mount.read(path).unwrap())
                    .unwrap_or_else(|e| panic!("{}: {}", path, e));
                n += 1;
            }
        }
        assert_eq!(n, 2591 + 715 + 1751 + 2399);
    }
}
