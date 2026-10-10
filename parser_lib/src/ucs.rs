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
