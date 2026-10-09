//! Lossless, bounds checked parsing of the part of a DOW2 replay that
//! precedes the tick stream.
//!
//! The original parser treated the archive header as a 36 byte gap and then
//! parsed chunks directly from the file cursor.  That happened to work for
//! the samples in the repository, but made a changed archive header or an
//! unknown chunk indistinguishable from a corrupt replay.  This module keeps
//! the bytes and offsets while exposing the fields whose widths are known.

use byteorder::{ByteOrder, LittleEndian};
use std::io::{self, Error, ErrorKind};

pub const REPLAY_HEADER_SIZE: usize = 84;
pub const ARCHIVE_HEADER_SIZE: usize = 36;
pub const CHUNK_HEADER_SIZE: usize = 28;
pub const CHUNKY_SIGNATURE: &[u8; 16] = b"Relic Chunky\r\n\x1a\0";

#[derive(Clone, Debug, Serialize)]
pub struct ReplayHeader {
    pub version: u32,
    pub checksum: u32,
    pub reserved: u32,
    pub magic: Vec<u8>,
    pub date: String,
    pub date_units: Vec<u16>,
    pub raw: Vec<u8>,
}

impl ReplayHeader {
    pub fn parse(bytes: &[u8]) -> io::Result<Self> {
        if bytes.len() < REPLAY_HEADER_SIZE {
            return Err(invalid("replay header is truncated"));
        }
        if &bytes[12..20] != b"DOW2_REC" {
            return Err(invalid("invalid replay magic"));
        }

        let mut date_units = Vec::with_capacity(32);
        for i in 0..32 {
            date_units.push(LittleEndian::read_u16(&bytes[20 + i * 2..22 + i * 2]));
        }
        let date = String::from_utf16_lossy(&date_units)
            .trim_end_matches('\0')
            .to_owned();

        Ok(Self {
            version: LittleEndian::read_u32(&bytes[0..4]),
            checksum: LittleEndian::read_u32(&bytes[4..8]),
            reserved: LittleEndian::read_u32(&bytes[8..12]),
            magic: bytes[12..20].to_vec(),
            date,
            date_units,
            raw: bytes[..REPLAY_HEADER_SIZE].to_vec(),
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ArchiveHeader {
    pub offset: u64,
    pub signature: Vec<u8>,
    pub fields: Vec<u8>,
    pub raw: Vec<u8>,
}

impl ArchiveHeader {
    pub fn parse(bytes: &[u8], offset: usize) -> io::Result<Self> {
        let end = offset
            .checked_add(ARCHIVE_HEADER_SIZE)
            .ok_or_else(|| invalid("archive header offset overflow"))?;
        if end > bytes.len() {
            return Err(invalid("archive header is truncated"));
        }
        if &bytes[offset..offset + CHUNKY_SIGNATURE.len()] != CHUNKY_SIGNATURE {
            return Err(invalid_at("invalid Relic Chunky signature", offset));
        }
        Ok(Self {
            offset: offset as u64,
            signature: bytes[offset..offset + 16].to_vec(),
            fields: bytes[offset + 16..end].to_vec(),
            raw: bytes[offset..end].to_vec(),
        })
    }

    pub fn content_offset(&self) -> usize {
        self.offset as usize + ARCHIVE_HEADER_SIZE
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ChunkRecord {
    pub offset: u64,
    pub name: String,
    pub name_bytes: Vec<u8>,
    pub version: u32,
    pub size: u32,
    pub header_extra: Vec<u8>,
    pub payload: Vec<u8>,
    pub children: Vec<ChunkRecord>,
    /// Bytes left in a folder after its complete child headers.  Some
    /// versions may append folder-specific padding or fields; retaining this
    /// prevents a folder walk from silently discarding a short tail.
    pub children_tail: Vec<u8>,
}

impl ChunkRecord {
    pub fn end_offset(&self) -> u64 {
        self.offset + CHUNK_HEADER_SIZE as u64 + self.size as u64
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct MetadataScan {
    pub outer_archive: ArchiveHeader,
    pub outer_chunks: Vec<ChunkRecord>,
    pub inner_archive: ArchiveHeader,
    pub chunks: Vec<ChunkRecord>,
    pub end_offset: u64,
}

/// Parse the nested archive and metadata chunks at the beginning of a replay.
///
/// The outer archive contains FOLDPOST, whose end is the offset of the inner
/// archive.  The inner archive has no root-size field; FOLDINFO is the final
/// metadata folder in observed DOW2 files, so this function stops immediately
/// after that folder and returns the action-stream offset.  Every chunk's
/// header, payload, and nested folder children are retained verbatim.
pub fn scan_metadata(bytes: &[u8]) -> io::Result<MetadataScan> {
    let outer_archive = ArchiveHeader::parse(bytes, REPLAY_HEADER_SIZE)?;
    // The outer archive has no root-size field either, but its first chunk is
    // FOLDPOST and the end of that chunk is the next archive header.  Parsing
    // the whole file as one outer sequence would mistake the inner signature
    // (and eventually action records) for chunk headers.
    let outer_chunks = vec![scan_chunk(
        bytes,
        outer_archive.content_offset(),
        bytes.len(),
        true,
    )?];
    let outer = outer_chunks
        .first()
        .ok_or_else(|| invalid("missing outer metadata chunk"))?;
    if outer.name != "FOLDPOST" {
        return Err(invalid_at(
            "outer metadata does not start with FOLDPOST",
            outer.offset as usize,
        ));
    }

    let inner_offset = outer.end_offset() as usize;
    let inner_archive = ArchiveHeader::parse(bytes, inner_offset)?;
    let mut chunks = Vec::new();
    let mut cursor = inner_archive.content_offset();
    let mut foldinfo_end = None;
    while cursor < bytes.len() {
        let chunk = scan_chunk(bytes, cursor, bytes.len(), true)?;
        cursor = chunk.end_offset() as usize;
        if chunk.name == "FOLDINFO" {
            foldinfo_end = Some(cursor);
            chunks.push(chunk);
            break;
        }
        chunks.push(chunk);
    }
    let end_offset = foldinfo_end.ok_or_else(|| invalid("metadata has no FOLDINFO folder"))?;

    Ok(MetadataScan {
        outer_archive,
        outer_chunks,
        inner_archive,
        chunks,
        end_offset: end_offset as u64,
    })
}

/// Parse one chunk at `offset`, checking all arithmetic against `bound`.
pub fn scan_chunk(
    bytes: &[u8],
    offset: usize,
    bound: usize,
    recurse: bool,
) -> io::Result<ChunkRecord> {
    let header_end = offset
        .checked_add(CHUNK_HEADER_SIZE)
        .ok_or_else(|| invalid("chunk header offset overflow"))?;
    if header_end > bound || header_end > bytes.len() {
        return Err(invalid_at("truncated chunk header", offset));
    }
    let name_bytes = bytes[offset..offset + 8].to_vec();
    let name = String::from_utf8_lossy(&name_bytes)
        .trim_end_matches('\0')
        .to_owned();
    let version = LittleEndian::read_u32(&bytes[offset + 8..offset + 12]);
    let size = LittleEndian::read_u32(&bytes[offset + 12..offset + 16]);
    let payload_end = header_end
        .checked_add(size as usize)
        .ok_or_else(|| invalid("chunk size overflow"))?;
    if payload_end > bound || payload_end > bytes.len() {
        return Err(invalid_at("chunk payload exceeds containing bound", offset));
    }
    let header_extra = bytes[offset + 16..header_end].to_vec();
    let payload = bytes[header_end..payload_end].to_vec();
    let (children, children_tail) = if recurse && name.starts_with("FOLD") {
        scan_children(bytes, header_end, payload_end, true)?
    } else {
        (Vec::new(), Vec::new())
    };

    Ok(ChunkRecord {
        offset: offset as u64,
        name,
        name_bytes,
        version,
        size,
        header_extra,
        payload,
        children,
        children_tail,
    })
}

/// Parse contiguous chunks until `bound`. A short tail is kept by the caller
/// as opaque data; a complete header with an out-of-bounds size is rejected.
pub fn scan_chunk_sequence(
    bytes: &[u8],
    offset: usize,
    bound: usize,
    recurse: bool,
) -> io::Result<Vec<ChunkRecord>> {
    let (chunks, tail) = scan_children(bytes, offset, bound, recurse)?;
    if !tail.is_empty() {
        return Err(invalid_at(
            "short opaque tail in chunk sequence",
            bound - tail.len(),
        ));
    }
    Ok(chunks)
}

fn scan_children(
    bytes: &[u8],
    mut offset: usize,
    bound: usize,
    recurse: bool,
) -> io::Result<(Vec<ChunkRecord>, Vec<u8>)> {
    if bound > bytes.len() || offset > bound {
        return Err(invalid("invalid chunk sequence bounds"));
    }
    let mut chunks = Vec::new();
    while bound - offset >= CHUNK_HEADER_SIZE {
        let chunk = scan_chunk(bytes, offset, bound, recurse)?;
        offset = chunk.end_offset() as usize;
        chunks.push(chunk);
    }
    Ok((chunks, bytes[offset..bound].to_vec()))
}

/// Read a length-prefixed byte string from a bounded slice. The length is in
/// bytes, unlike the UTF-16 helper below where it is in 16-bit code units.
pub fn read_vstring(bytes: &[u8], offset: &mut usize) -> io::Result<Vec<u8>> {
    let len = read_u32(bytes, offset)? as usize;
    let end = offset
        .checked_add(len)
        .ok_or_else(|| invalid("vstring length overflow"))?;
    if end > bytes.len() {
        return Err(invalid_at("vstring exceeds payload", *offset));
    }
    let value = bytes[*offset..end].to_vec();
    *offset = end;
    Ok(value)
}

/// Read a length-prefixed UTF-16LE string. The length is a count of u16 code
/// units. Invalid UTF-16 is represented lossily while the original chunk
/// payload remains available through `ChunkRecord::payload`.
pub fn read_vstring_utf16(bytes: &[u8], offset: &mut usize) -> io::Result<String> {
    let count = read_u32(bytes, offset)? as usize;
    let byte_len = count
        .checked_mul(2)
        .ok_or_else(|| invalid("UTF-16 string length overflow"))?;
    let end = offset
        .checked_add(byte_len)
        .ok_or_else(|| invalid("UTF-16 string offset overflow"))?;
    if end > bytes.len() {
        return Err(invalid_at("UTF-16 string exceeds payload", *offset));
    }
    let mut units = Vec::with_capacity(count);
    for chunk in bytes[*offset..end].chunks_exact(2) {
        units.push(LittleEndian::read_u16(chunk));
    }
    *offset = end;
    Ok(String::from_utf16_lossy(&units))
}

fn read_u32(bytes: &[u8], offset: &mut usize) -> io::Result<u32> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| invalid("u32 offset overflow"))?;
    if end > bytes.len() {
        return Err(invalid_at("truncated u32", *offset));
    }
    let value = LittleEndian::read_u32(&bytes[*offset..end]);
    *offset = end;
    Ok(value)
}

fn invalid(message: &str) -> io::Error {
    Error::new(ErrorKind::InvalidData, message)
}

fn invalid_at(message: &str, offset: usize) -> io::Error {
    Error::new(ErrorKind::InvalidData, format!("{} at {}", message, offset))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryInto;

    fn chunk(name: &[u8; 8], version: u32, payload: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(name);
        bytes.extend_from_slice(&version.to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&[0xA5; 12]);
        bytes.extend_from_slice(payload);
        bytes
    }

    #[test]
    fn chunk_scan_preserves_header_extra_and_nested_unknown_bytes() {
        let child = chunk(b"UNKNOWN!", 17, &[1, 2, 3]);
        let parent = chunk(b"FOLDTEST", 2, &child);
        let parsed = scan_chunk(&parent, 0, parent.len(), true).unwrap();
        assert_eq!(parsed.header_extra, vec![0xA5; 12]);
        assert_eq!(parsed.children.len(), 1);
        assert_eq!(parsed.children[0].name, "UNKNOWN!");
        assert_eq!(parsed.children[0].payload, vec![1, 2, 3]);
        assert!(parsed.children_tail.is_empty());

        let mut with_tail = parent.clone();
        let parent_size = u32::from_le_bytes(with_tail[12..16].try_into().unwrap());
        with_tail[12..16].copy_from_slice(&(parent_size + 1).to_le_bytes());
        with_tail.push(0x7f);
        let parsed = scan_chunk(&with_tail, 0, with_tail.len(), true).unwrap();
        assert_eq!(parsed.children_tail, vec![0x7f]);
    }

    #[test]
    fn chunk_scan_rejects_payload_beyond_bound() {
        let mut bytes = chunk(b"DATADATA", 1, &[1, 2]);
        bytes[12..16].copy_from_slice(&100u32.to_le_bytes());
        assert!(scan_chunk(&bytes, 0, bytes.len(), false).is_err());
    }

    #[test]
    fn replay_header_keeps_all_utf16_units() {
        let mut bytes = vec![0u8; REPLAY_HEADER_SIZE];
        bytes[12..20].copy_from_slice(b"DOW2_REC");
        bytes[20..22].copy_from_slice(&(0x00c4u16).to_le_bytes());
        bytes[22..24].copy_from_slice(&(0x03a9u16).to_le_bytes());
        let header = ReplayHeader::parse(&bytes).unwrap();
        assert_eq!(header.date_units[0], 0x00c4);
        assert_eq!(header.date_units[1], 0x03a9);
        assert_eq!(header.date, "ÄΩ");
        assert_eq!(header.raw.len(), REPLAY_HEADER_SIZE);
    }

    #[test]
    fn strings_are_bounded() {
        let mut bytes = 4u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&[1, 2]);
        assert!(read_vstring(&bytes, &mut 0).is_err());
        let mut bytes = 2u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&[0x41, 0]);
        assert!(read_vstring_utf16(&bytes, &mut 0).is_err());
    }

    #[test]
    fn repository_replays_have_bounded_metadata() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        for path in [
            root.join("3v3.rec"),
            root.join("recs/1.rec"),
            root.join("recs/purchases_SM.rec"),
            root.join("recs/upgrades.rec"),
            root.join("recs/unit_transformation_and_abilities.rec"),
        ] {
            let bytes = std::fs::read(&path).unwrap();
            let header = ReplayHeader::parse(&bytes).unwrap();
            assert_eq!(header.raw.len(), REPLAY_HEADER_SIZE);
            let scan = scan_metadata(&bytes).unwrap();
            assert_eq!(scan.outer_chunks[0].name, "FOLDPOST");
            assert_eq!(scan.chunks.last().unwrap().name, "FOLDINFO");
            assert!(scan.end_offset as usize <= bytes.len());
        }
    }
}
