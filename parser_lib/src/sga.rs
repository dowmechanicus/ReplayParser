use byteorder::{LittleEndian, ReadBytesExt};
use std::fs::File;
use std::io::{self, BufReader, Cursor, Error, ErrorKind, Read, Seek, SeekFrom};
use std::path::Path;

const MAGIC: &[u8; 8] = b"_ARCHIVE";
const DATA_START_FIELD: u64 = 0xB0;
const TOC_HEADER_LEN: u64 = 24;
const FOLDER_LEN: u64 = 12;
const FILE_LEN: u64 = 22;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Storage {
    Stored,
    Zlib,
    Unknown(u8),
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub path: String,
    /// Absolute file offset of the entry's data.
    pub data_offset: u64,
    pub compressed_len: u32,
    pub uncompressed_len: u32,
    pub storage: Storage,
}

#[derive(Debug)]
pub struct Archive {
    pub entries: Vec<Entry>,
}

fn invalid(msg: String) -> Error {
    Error::new(ErrorKind::InvalidData, msg)
}

/// Reads `count` records of `size` bytes at `start`, checking the table lies within the stream.
fn read_table<R: Read + Seek>(
    r: &mut R,
    start: u64,
    count: u16,
    size: u64,
    len: u64,
    what: &str,
) -> io::Result<Vec<u8>> {
    let total = u64::from(count) * size;
    if start > len || total > len - start {
        return Err(invalid(format!("{} table out of bounds", what)));
    }
    r.seek(SeekFrom::Start(start))?;
    let mut buf = vec![0; total as usize];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

fn name_at(names: &[u8], offset: u32) -> io::Result<String> {
    let rest = names
        .get(offset as usize..)
        .ok_or_else(|| invalid(format!("name offset {} out of bounds", offset)))?;
    let end = rest
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| invalid(format!("name at offset {} is not terminated", offset)))?;
    Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
}

impl Archive {
    pub fn open(path: &Path) -> io::Result<Archive> {
        Archive::from_reader(BufReader::new(File::open(path)?))
    }

    pub fn from_reader<R: Read + Seek>(mut r: R) -> io::Result<Archive> {
        let len = r.seek(SeekFrom::End(0))?;
        r.seek(SeekFrom::Start(0))?;
        let mut magic = [0u8; 8];
        r.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(invalid("not an SGA archive (bad magic)".to_string()));
        }
        let major = r.read_u16::<LittleEndian>()?;
        let _minor = r.read_u16::<LittleEndian>()?;
        if major != 5 {
            return Err(invalid(format!("unsupported SGA version {}", major)));
        }

        r.seek(SeekFrom::Start(DATA_START_FIELD))?;
        let data_start = r.read_u32::<LittleEndian>()?;
        let toc_offset = u64::from(r.read_u32::<LittleEndian>()?);
        if toc_offset > len || TOC_HEADER_LEN > len - toc_offset {
            return Err(invalid(format!("toc offset {} out of bounds", toc_offset)));
        }

        r.seek(SeekFrom::Start(toc_offset))?;
        let _drive_off = r.read_u32::<LittleEndian>()?;
        let _drive_count = r.read_u16::<LittleEndian>()?;
        let folder_off = r.read_u32::<LittleEndian>()?;
        let folder_count = r.read_u16::<LittleEndian>()?;
        let file_off = r.read_u32::<LittleEndian>()?;
        let file_count = r.read_u16::<LittleEndian>()?;
        let name_off = r.read_u32::<LittleEndian>()?;
        let _name_count = r.read_u16::<LittleEndian>()?;

        let folders = read_table(
            &mut r,
            toc_offset + u64::from(folder_off),
            folder_count,
            FOLDER_LEN,
            len,
            "folder",
        )?;
        let files = read_table(
            &mut r,
            toc_offset + u64::from(file_off),
            file_count,
            FILE_LEN,
            len,
            "file",
        )?;

        // The name block has no recorded size: it runs to the start of the file data when that
        // follows it, otherwise to the end of the stream.
        let names_start = toc_offset + u64::from(name_off);
        if names_start > len {
            return Err(invalid("name table out of bounds".to_string()));
        }
        let names_end = if u64::from(data_start) > names_start && u64::from(data_start) <= len {
            u64::from(data_start)
        } else {
            len
        };
        r.seek(SeekFrom::Start(names_start))?;
        let mut names = vec![0; (names_end - names_start) as usize];
        r.read_exact(&mut names)?;

        let mut folder_cur = Cursor::new(folders);
        let mut file_cur = Cursor::new(files);
        let mut entries = Vec::new();
        for _ in 0..folder_count {
            let folder_name = name_at(&names, folder_cur.read_u32::<LittleEndian>()?)?;
            let _first_subfolder = folder_cur.read_u16::<LittleEndian>()?;
            let _last_subfolder = folder_cur.read_u16::<LittleEndian>()?;
            let first_file = folder_cur.read_u16::<LittleEndian>()?;
            let last_file = folder_cur.read_u16::<LittleEndian>()?;
            if first_file > last_file || last_file > file_count {
                return Err(invalid(format!(
                    "folder file range {}..{} out of bounds ({} files)",
                    first_file, last_file, file_count
                )));
            }

            for index in first_file..last_file {
                file_cur.set_position(u64::from(index) * FILE_LEN);
                let file_name = name_at(&names, file_cur.read_u32::<LittleEndian>()?)?;
                let data_offset = file_cur.read_u32::<LittleEndian>()?;
                let compressed_len = file_cur.read_u32::<LittleEndian>()?;
                let uncompressed_len = file_cur.read_u32::<LittleEndian>()?;
                let _mtime = file_cur.read_u32::<LittleEndian>()?;
                let _flag = file_cur.read_u8()?;
                let storage = storage_from_byte(file_cur.read_u8()?);
                let path = if folder_name.is_empty() {
                    file_name
                } else {
                    format!("{}\\{}", folder_name, file_name)
                };
                entries.push(Entry {
                    path,
                    data_offset: u64::from(data_start) + u64::from(data_offset),
                    compressed_len,
                    uncompressed_len,
                    storage,
                });
            }
        }
        Ok(Archive { entries })
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    const TOC: usize = 0xB8;
    const DATA_START: u32 = 0x400;

    fn put_u16(out: &mut Vec<u8>, v: u16) {
        out.extend_from_slice(&v.to_le_bytes());
    }

    fn put_u32(out: &mut Vec<u8>, v: u32) {
        out.extend_from_slice(&v.to_le_bytes());
    }

    /// Builds an archive with folder `a\b` (files x.rbf, y.rbf) and an unnamed folder (z.txt).
    fn build(magic: &[u8; 8], major: u16, bad_name: bool, last_file: u16) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(magic);
        put_u16(&mut out, major);
        put_u16(&mut out, 0);
        out.resize(0xB0, 0);
        put_u32(&mut out, DATA_START);
        put_u32(&mut out, TOC as u32);
        assert_eq!(out.len(), TOC);

        let folder_off = 24u32;
        let file_off = folder_off + 2 * 12;
        let name_off = file_off + 3 * 22;
        // header: drive table is empty
        put_u32(&mut out, folder_off);
        put_u16(&mut out, 0);
        put_u32(&mut out, folder_off);
        put_u16(&mut out, 2);
        put_u32(&mut out, file_off);
        put_u16(&mut out, 3);
        put_u32(&mut out, name_off);
        put_u16(&mut out, 5);

        // names: "a\b", "", "x.rbf", "y.rbf", "z.txt"
        let names = b"a\\b\0\0x.rbf\0y.rbf\0z.txt\0";
        let (n_folder_a, n_folder_empty, n_x, n_y, n_z) = (0u32, 4u32, 5u32, 11u32, 17u32);

        for &(name, first, last) in &[(n_folder_a, 0u16, 2u16), (n_folder_empty, 2, last_file)] {
            put_u32(&mut out, name);
            put_u16(&mut out, 0);
            put_u16(&mut out, 0);
            put_u16(&mut out, first);
            put_u16(&mut out, last);
        }

        let z_name = if bad_name { 0xFFFF } else { n_z };
        for &(name, data, clen, ulen, storage) in &[
            (n_x, 0u32, 10u32, 20u32, 1u8),
            (n_y, 10, 30, 30, 0),
            (z_name, 40, 5, 5, 7),
        ] {
            put_u32(&mut out, name);
            put_u32(&mut out, data);
            put_u32(&mut out, clen);
            put_u32(&mut out, ulen);
            put_u32(&mut out, 0);
            out.push(0);
            out.push(storage);
        }
        out.extend_from_slice(names);
        out
    }

    fn parse(bytes: Vec<u8>) -> io::Result<Archive> {
        Archive::from_reader(Cursor::new(bytes))
    }

    #[test]
    fn parses_entries() {
        let archive = parse(build(MAGIC, 5, false, 3)).unwrap();
        let paths: Vec<&str> = archive.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, vec!["a\\b\\x.rbf", "a\\b\\y.rbf", "z.txt"]);

        let offsets: Vec<u64> = archive.entries.iter().map(|e| e.data_offset).collect();
        let base = u64::from(DATA_START);
        assert_eq!(offsets, vec![base, base + 10, base + 40]);

        let lens: Vec<(u32, u32)> = archive
            .entries
            .iter()
            .map(|e| (e.compressed_len, e.uncompressed_len))
            .collect();
        assert_eq!(lens, vec![(10, 20), (30, 30), (5, 5)]);

        let storage: Vec<Storage> = archive.entries.iter().map(|e| e.storage).collect();
        assert_eq!(
            storage,
            vec![Storage::Zlib, Storage::Stored, Storage::Unknown(7)]
        );
    }

    #[test]
    fn rejects_bad_magic() {
        assert!(parse(build(b"_ARCHIVX", 5, false, 3)).is_err());
    }

    #[test]
    fn rejects_version_4() {
        let err = parse(build(MAGIC, 4, false, 3)).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert!(err.to_string().contains('4'));
    }

    #[test]
    fn rejects_name_offset_past_block() {
        let err = parse(build(MAGIC, 5, true, 3)).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }

    #[test]
    fn rejects_last_file_past_count() {
        let err = parse(build(MAGIC, 5, false, 4)).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }

    #[test]
    fn rejects_toc_offset_past_eof() {
        let mut bytes = build(MAGIC, 5, false, 3);
        bytes[0xB4..0xB8].copy_from_slice(&0x00FF_FFFFu32.to_le_bytes());
        let err = parse(bytes).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }

    #[test]
    #[ignore]
    fn real_archives_sbp_counts() {
        let install = match env::var("DOW2_INSTALL") {
            Ok(v) => v,
            Err(_) => return,
        };
        for &(name, expected) in &[("Elite_attrib.sga", 288), ("GameAttrib_orig.sga", 441)] {
            let path = Path::new(&install).join("Elite/Archives").join(name);
            let archive = Archive::open(&path).unwrap();
            let count = archive
                .entries
                .iter()
                .filter(|e| {
                    let p = e.path.to_lowercase();
                    p.contains("simulation\\attrib\\sbps\\") && p.ends_with(".rbf")
                })
                .count();
            assert_eq!(count, expected, "{}", name);
        }
    }

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
        assert_eq!(
            entry.read(&mut Cursor::new(bytes)).unwrap(),
            payload.to_vec()
        );
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
        assert_eq!(
            entry.read(&mut Cursor::new(bytes)).unwrap(),
            b"abcd".to_vec()
        );
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
            .find(|e| {
                e.path
                    .to_ascii_lowercase()
                    .ends_with("ebps\\pvp\\race_marine\\buildings\\sm_generator.rbf")
            })
            .unwrap();
        assert_eq!((entry.compressed_len, entry.uncompressed_len), (2521, 5913));
        assert_eq!(entry.storage, Storage::Zlib);
        let data = entry.read(&mut File::open(&path).unwrap()).unwrap();
        assert_eq!(data.len(), 5913);
        assert_eq!(&data[..8], b"RBF V0.1");
    }
}
