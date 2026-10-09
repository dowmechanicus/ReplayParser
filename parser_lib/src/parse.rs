use byteorder::{ByteOrder, LittleEndian, ReadBytesExt};
use crypto::digest::Digest;
use crypto::md5::Md5;

use chunky::Chunk;
use chunky::Data as DataChunk;
use chunky::Player as PlayerChunk;

use crate::message::Message;
use crate::replay::{ActionBundle, RawRecord, ReplayInfo, SyncRecord};

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Cursor, Error, ErrorKind, Read, Seek, SeekFrom};

use std::path::Path;

use crate::actions::Action;
use crate::chunky;

const TICK_ACTION: u32 = 0;
const TICK_CHATMSG: u32 = 1;

fn read_rec_file(path: &Path) -> Result<Vec<u8>, io::Error> {
    let mut file = File::open(path)?;
    let mut buf = [0; 20];
    let mut vec = Vec::new();

    file.read_exact(&mut buf)?;

    if buf[12..20].eq(b"DOW2_REC") {
        file.seek(SeekFrom::Start(0))?;
        file.read_to_end(&mut vec)?;
        Ok(vec)
    } else {
        Err(Error::new(ErrorKind::InvalidData, "invalid replay file"))
    }
}

pub fn parse_replay(path: &Path) -> Result<ReplayInfo, io::Error> {
    let bytes = read_rec_file(path)?;
    let metadata = crate::metadata::scan_metadata(&bytes)?;
    let action_offset = metadata.end_offset;
    let header = &metadata.header;
    let mut cursor = Cursor::new(bytes);

    let version = header.version;
    let mod_chksum = header.checksum;
    cursor.set_position(crate::metadata::REPLAY_HEADER_SIZE as u64);

    let mut replay = ReplayInfo {
        mod_chksum,
        mod_version: version,
        date: header.date.clone(),
        metadata: Some(metadata),
        ..Default::default()
    };

    let mut digest = Md5::new();
    digest.input(cursor.get_ref());
    replay.md5 = digest.result_str();

    let mut buf = vec![0; 12];
    cursor.read_exact(&mut buf)?;
    //let file_format = String::from_utf8(buf).unwrap_or("".to_string());

    cursor.seek(SeekFrom::Current(24))?;

    parse_chunks(&mut cursor, &mut replay, action_offset)?;
    let replay_len = cursor.get_ref().len() as u64;
    parse_ticks(&mut cursor, &mut replay, replay_len)?;

    match_player_ids_from_messages(&mut replay);

    Ok(replay)
}

pub fn parse_chunks(
    cursor: &mut Cursor<Vec<u8>>,
    replay: &mut ReplayInfo,
    pos: u64,
) -> Result<(), io::Error> {
    if cursor.position() >= pos {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "metadata ends before chunk stream",
        ));
    }
    chunky::parse(cursor)?;
    if let Chunk::Data(DataChunk { duration }) = chunky::parse(cursor)? {
        replay.ticks = duration;
    }
    if cursor.position() > pos {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "legacy chunk parser crossed metadata boundary",
        ));
    }
    let archive_offset = cursor.position() as usize;
    let bytes = cursor.get_ref();
    let archive_end = archive_offset
        .checked_add(crate::metadata::ARCHIVE_HEADER_SIZE)
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "inner archive offset overflow"))?;
    if archive_end > bytes.len()
        || &bytes[archive_offset..archive_offset + crate::metadata::CHUNKY_SIGNATURE.len()]
            != crate::metadata::CHUNKY_SIGNATURE
    {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "inner archive header is missing",
        ));
    }
    cursor.set_position(archive_end as u64);

    let mut endpos = pos;
    loop {
        if cursor.position() >= endpos {
            break; // end of header chunks, start of actions
        }

        let chunk_start = cursor.position() as usize;
        if pos.saturating_sub(cursor.position()) < 28 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "truncated metadata chunk header",
            ));
        }
        let bytes = cursor.get_ref();
        let name = String::from_utf8_lossy(&bytes[chunk_start..chunk_start + 8]).to_string();
        let chunk_size = LittleEndian::read_u32(&bytes[chunk_start + 12..chunk_start + 16]) as u64;
        let chunk_end = cursor
            .position()
            .checked_add(28)
            .and_then(|end| end.checked_add(chunk_size))
            .filter(|end| *end <= pos && *end <= bytes.len() as u64)
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, "metadata chunk exceeds boundary"))?;

        // Metadata scanning already retained unknown chunks byte-for-byte.
        // Skip them here so the legacy semantic view cannot make an otherwise
        // valid replay unparseable merely because a mod added a chunk name.
        let known = matches!(
            name.as_str(),
            "DATADATA"
                | "DATASDSC"
                | "DATABASE"
                | "DATAINFO"
                | "FOLDINFO"
                | "FOLDPOST"
                | "FOLDGPLY"
        );
        if !known {
            cursor.set_position(chunk_end);
            continue;
        }

        let parsed = match chunky::parse(cursor) {
            Ok(chunk) => chunk,
            Err(_) => {
                // The bounded metadata scan has retained this chunk's raw
                // bytes. Keep the semantic projection optional when a known
                // chunk's payload layout changes across versions.
                cursor.set_position(chunk_end);
                continue;
            }
        };
        match parsed {
            Chunk::Empty { .. } => (),
            Chunk::FoldInfo { size } => endpos = cursor.position() + size as u64,
            Chunk::Data(DataChunk { duration }) => replay.ticks = duration,
            c @ Chunk::Map { .. } => replay.map = c,
            c @ Chunk::Game { .. } => replay.game = c,
            c @ Chunk::Player { .. } => {
                if let Chunk::Player(PlayerChunk { kind, .. }) = c {
                    if kind == 2 || kind == 5 {
                        replay.observers.push(c)
                    } else if kind != 7 {
                        replay.players.push(c)
                    }
                }
            }
        };
        if cursor.position() > chunk_end {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "legacy chunk parser crossed chunk boundary",
            ));
        }
    }

    Ok(())
}
pub fn parse_ticks(
    cursor: &mut Cursor<Vec<u8>>,
    replay: &mut ReplayInfo,
    pos: u64,
) -> Result<(), io::Error> {
    let mut current_tick = 0;
    loop {
        if cursor.position() >= pos {
            break;
        }

        let tick_type = cursor.read_u32::<LittleEndian>()?;
        let tick_size = cursor.read_u32::<LittleEndian>()? as u64;
        let record_end = cursor
            .position()
            .checked_add(tick_size)
            .filter(|end| *end <= pos && *end <= cursor.get_ref().len() as u64)
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, "record exceeds replay bounds"))?;
        // Parse a bounded payload so malformed commands cannot consume the next record.
        let start = cursor.position() as usize;
        let mut payload = Cursor::new(cursor.get_ref()[start..record_end as usize].to_vec());
        cursor.set_position(record_end);

        match tick_type {
            TICK_ACTION => {
                match parse_action_record(&mut payload) {
                    Ok(record) => {
                        let tick = record.tick;

                        if tick > 0 {
                            current_tick = tick
                        }

                        for bundle in &record.bundles {
                            for action in &bundle.commands {
                                replay.commands.push(action.clone());
                                if action.data[0] != 44
                                    && action.data[0] != 11 // set rally point
                                    && action.data[0] != 23 // exit building
                                    && action.data[0] != 43 // stop move
                                    && action.data[0] != 47 // capture point
                                    && action.data[0] != 48 // attack
                                    && action.data[0] != 49 // reinforce
                                    && action.data[0] != 52 // attack move
                                    && action.data[0] != 53 // ability on unit
                                    && action.data[0] != 56 // enter building or vehicle
                                    && action.data[0] != 58 // exit vehicle
                                    && action.data[0] != 61 // retreat
                                    && action.data[0] != 70 // force melee
                                    && action.data[0] != 71
                                // toggle stance
                                {
                                    replay.actions.push(action.clone());
                                }
                            }
                        }
                        replay.sync_records.push(record);
                    }
                    Err(error) => {
                        let raw = payload.get_ref().clone();
                        payload.set_position(tick_size);
                        replay.unknown_records.push(RawRecord {
                            record_type: tick_type,
                            payload: raw,
                            error: Some(error.to_string()),
                        });
                    }
                }
            }
            TICK_CHATMSG => match parse_message(&mut payload, current_tick) {
                Ok(msg) => replay.messages.push(msg),
                Err(error) => {
                    let raw = payload.get_ref().clone();
                    payload.set_position(tick_size);
                    replay.unknown_records.push(RawRecord {
                        record_type: tick_type,
                        payload: raw,
                        error: Some(error.to_string()),
                    });
                }
            },
            _ => {
                replay.unknown_records.push(RawRecord {
                    record_type: tick_type,
                    payload: payload.get_ref().clone(),
                    error: None,
                });
                continue;
            }
        };
        if payload.position() != tick_size {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "record payload length mismatch",
            ));
        }
    }

    Ok(())
}

/// Read a synchronization record payload (the outer type/length are already removed).
/// Bundles contain a u64 metadata value, u32 byte length, one redundant low length
/// byte, then commands framed by a little-endian u16 length including that u16.
#[allow(dead_code)]
pub fn parse_action(cursor: &mut Cursor<Vec<u8>>) -> Result<(Vec<Action>, u32), io::Error> {
    let record = parse_action_record(cursor)?;
    let tick = record.tick;
    let actions = record
        .bundles
        .into_iter()
        .flat_map(|bundle| bundle.commands)
        .collect();
    Ok((actions, tick))
}

/// Parse a synchronization payload while retaining its prefix and bundle metadata.
pub fn parse_action_record(cursor: &mut Cursor<Vec<u8>>) -> Result<SyncRecord, io::Error> {
    let raw_payload = cursor.get_ref().clone();
    let marker = cursor.read_u8()?; // synchronization message tag (0x20 in inspected files)
    if marker != 0x20 {
        return Err(Error::new(ErrorKind::InvalidData, "invalid sync marker"));
    }
    let tick = cursor.read_u32::<LittleEndian>()?;
    let counter = cursor.read_u32::<LittleEndian>()?;
    let unknown = cursor.read_u32::<LittleEndian>()?;
    let nbundles = cursor.read_u32::<LittleEndian>()?;
    let mut bundles = Vec::new();

    for _ in 0..nbundles {
        let metadata = cursor.read_u64::<LittleEndian>()?;
        let bundle_size = cursor.read_u32::<LittleEndian>()? as u64;
        let length_low = cursor.read_u8()?;
        if length_low != bundle_size as u8 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "bundle length echo mismatch",
            ));
        }
        let bundle_end = cursor
            .position()
            .checked_add(bundle_size)
            .filter(|end| *end <= cursor.get_ref().len() as u64)
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, "bundle exceeds record bounds"))?;

        let mut actions = Vec::new();
        while cursor.position() < bundle_end {
            let remaining = bundle_end - cursor.position();
            if remaining < 2 {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "truncated command length",
                ));
            }
            let command_size = cursor.read_u16::<LittleEndian>()? as u64;
            // Minimum body: opcode, sender/flags, four reserved bytes, receiver
            // count (possibly zero), custom-data type (possibly 0xff).
            if command_size < 10 || command_size > remaining {
                return Err(Error::new(ErrorKind::InvalidData, "invalid command length"));
            }
            let mut data = vec![0; (command_size - 2) as usize];
            cursor.read_exact(&mut data)?;
            match crate::actions::Command::decode(&data) {
                Ok(command) => actions.push(Action::from_decoded(data, tick, command)),
                Err(error) => actions.push(Action::from_raw(data, tick, error.to_string())),
            }
        }
        bundles.push(ActionBundle {
            metadata,
            declared_size: bundle_size as u32,
            length_echo: length_low,
            commands: actions,
        });
    }
    if cursor.position() != cursor.get_ref().len() as u64 {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "sync payload has trailing bytes",
        ));
    }
    Ok(SyncRecord {
        marker,
        tick,
        counter,
        unknown,
        raw_payload,
        bundles,
    })
}

pub fn parse_message(cursor: &mut Cursor<Vec<u8>>, tick: u32) -> Result<Message, io::Error> {
    let raw = cursor.get_ref().clone();
    let mut offset = 0usize;
    let wire_kind = crate::metadata::read_u32(&raw, &mut offset)?;
    let wire_size = crate::metadata::read_u32(&raw, &mut offset)?;
    if wire_size as usize != raw.len().saturating_sub(8) {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "chat payload size field does not match record",
        ));
    }
    let sender = crate::metadata::read_vstring_utf16(&raw, &mut offset)?;
    if offset >= raw.len() {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "chat player id is missing",
        ));
    }
    let player_id = raw[offset];
    offset += 1;
    if raw.len().saturating_sub(offset) < 3 {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "chat player metadata is truncated",
        ));
    }
    let player_reserved = [raw[offset], raw[offset + 1], raw[offset + 2]];
    offset += 3;
    let kind = crate::metadata::read_u32(&raw, &mut offset)?;
    let local = crate::metadata::read_u32(&raw, &mut offset)?;
    let body = crate::metadata::read_vstring_utf16(&raw, &mut offset)?;
    if offset != raw.len() {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "chat payload has trailing bytes",
        ));
    }
    cursor.set_position(raw.len() as u64);

    let receiver = match local {
        1 if kind == 1 => "observers".to_string(),
        1 if kind != 1 => "team".to_string(),
        _ => "all".to_string(),
    };

    Ok(Message {
        tick,
        wire_kind,
        wire_size,
        sender,
        receiver,
        body,
        player_id,
        player_reserved,
        kind,
        local,
        raw,
    })
}

fn match_player_ids_from_messages(replay: &mut ReplayInfo) {
    let mut player_map = HashMap::new();
    let mut relic_id_map = HashMap::new();

    for message in replay.messages.iter() {
        player_map.insert(message.player_id, message.sender.clone());
    }

    for player in replay.players.iter() {
        if let Chunk::Player(p) = player {
            relic_id_map.insert(&p.name, p.relic_id);
        }
    }

    for action in replay.actions.iter_mut().chain(replay.commands.iter_mut()) {
        // data[1] carries the sender/slot flags. Message.player_id joins on
        // the low byte of the first metadata word at data[2..4] in the
        // inspected files; keep this compatibility key distinct from the
        // semantic sender field.
        match player_map.get(&action.data[2]) {
            Some(id) => {
                action.player = id.to_owned();
                if let Some(relic_id) = relic_id_map.get(id) {
                    action.relic_id = *relic_id;
                }
            }
            _ => (),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(body: &[u8]) -> Vec<u8> {
        let mut bytes = ((body.len() + 2) as u16).to_le_bytes().to_vec();
        bytes.extend_from_slice(body);
        bytes
    }

    fn synchronization(bundles: &[Vec<u8>]) -> Vec<u8> {
        let mut bytes = vec![0x20];
        for word in [11u32, 0, 0, bundles.len() as u32] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        for bundle in bundles {
            bytes.extend_from_slice(&0u64.to_le_bytes());
            bytes.extend_from_slice(&(bundle.len() as u32).to_le_bytes());
            bytes.push(bundle.len() as u8);
            bytes.extend_from_slice(bundle);
        }
        bytes
    }

    fn record(payload: &[u8]) -> Cursor<Vec<u8>> {
        let mut bytes = 0u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(payload);
        Cursor::new(bytes)
    }

    #[test]
    fn preserves_adjacent_commands_and_last_payload_byte() {
        let purchase = vec![3, 1, 233, 3, 0, 0, 16, 0, 120, 180, 5, 4, 141, 0, 0, 7];
        let retreat = vec![61, 1, 233, 3, 1, 0, 32, 0, 195, 83, 255];
        let mut bundle = command(&purchase);
        bundle.extend_from_slice(&command(&retreat));
        let payload = synchronization(&[bundle, command(&purchase)]);
        let mut cursor = Cursor::new(payload.clone());
        let (actions, tick) = parse_action(&mut cursor).unwrap();
        assert_eq!(tick, 11);
        assert_eq!(cursor.position(), payload.len() as u64);
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0].data, purchase);
        assert_eq!(actions[1].data, retreat);
        assert_eq!(actions[2].data, purchase);
        assert_eq!(actions[0].command.as_ref().unwrap().opcode, 3);
        assert_eq!(actions[1].command.as_ref().unwrap().custom_type, 0xff);
        let json = serde_json::to_value(&actions[0]).unwrap();
        assert_eq!(json["data"], serde_json::json!(purchase));
        assert_eq!(json["command"]["sender_slot"], 1);
    }

    #[test]
    fn retains_sync_prefix_and_bundle_metadata() {
        let purchase = vec![3, 1, 233, 3, 0, 0, 16, 0, 120, 180, 5, 4, 141, 0, 0, 7];
        let payload = synchronization(&[command(&purchase)]);
        let record = parse_action_record(&mut Cursor::new(payload)).unwrap();
        assert_eq!(record.marker, 0x20);
        assert_eq!(record.tick, 11);
        assert_eq!(record.counter, 0);
        assert_eq!(record.unknown, 0);
        assert_eq!(record.bundles.len(), 1);
        assert_eq!(record.bundles[0].metadata, 0);
        assert_eq!(record.bundles[0].declared_size, 18);
        assert_eq!(record.bundles[0].length_echo, 18);
        assert_eq!(record.bundles[0].commands.len(), 1);
    }

    #[test]
    fn retains_unknown_opcode_as_raw_and_structured_command() {
        let body = vec![250, 0, 232, 3, 0, 0, 0x40, 0xff];
        let payload = synchronization(&[command(&body)]);
        let (actions, _) = parse_action(&mut Cursor::new(payload)).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].data, body);
        assert_eq!(actions[0].command.as_ref().unwrap().opcode, 250);
    }

    #[test]
    fn decodes_receiver_encodings_and_custom_lengths() {
        let mut single = vec![250, 0x81, 1, 2, 3, 4];
        single.extend_from_slice(&0x1000_0007u32.to_be_bytes());
        single.extend_from_slice(&[7, 3, 1, 2, 3]);
        let decoded = crate::actions::Command::decode(&single).unwrap();
        assert_eq!(decoded.sender_slot, 1);
        assert_eq!(decoded.process_type, 1);
        assert_eq!(decoded.receivers.len(), 1);
        assert_eq!(decoded.receivers[0].kind, 1);
        assert_eq!(decoded.receivers[0].instance_id, 7);
        assert_eq!(decoded.custom_length, Some(3));
        assert_eq!(decoded.custom_data, vec![1, 2, 3]);

        for prefix in [
            vec![0x42, 1, 0, 0, 0, 2, 0, 0, 0],
            vec![0x80, 0x02, 1, 0, 0, 0, 2, 0, 0, 0],
            vec![0xc0, 0, 0, 2, 1, 0, 0, 0, 2, 0, 0, 0],
        ] {
            let mut body = vec![251, 0, 0, 0, 0, 0];
            body.extend_from_slice(&prefix);
            body.push(0xff);
            let decoded = crate::actions::Command::decode(&body).unwrap();
            assert_eq!(decoded.receivers.len(), 2);
            assert_eq!(decoded.receivers[0].instance_id, 1);
            assert_eq!(decoded.receivers[1].instance_id, 2);
            assert_eq!(decoded.custom_length, None);
        }

        let custom_size = 0x1234usize;
        let mut body = vec![252, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0x15, 0x92, 0x34];
        body.extend(std::iter::repeat(0xcd).take(custom_size));
        let decoded = crate::actions::Command::decode(&body).unwrap();
        assert_eq!(decoded.custom_length, Some(custom_size as u16));
        assert_eq!(decoded.custom_data.len(), custom_size);
    }

    #[test]
    fn accepts_lengths_over_255_and_wrapped_bundle_echo() {
        let mut body = vec![3, 0, 232, 3, 0, 0, 16, 0, 39, 16, 5, 0x81, 0];
        body.extend(std::iter::repeat(0xab).take(256));
        let payload = synchronization(&[command(&body)]);
        let (actions, _) = parse_action(&mut Cursor::new(payload)).unwrap();
        assert_eq!(actions[0].data, body);
    }

    #[test]
    fn rejects_invalid_lengths_without_panicking() {
        for size in [0u16, 1, 2, 9, 256, 65535] {
            let mut bytes = command(&[3, 0, 232, 3, 0, 0, 0x40, 0xff]);
            bytes[..2].copy_from_slice(&size.to_le_bytes());
            assert!(parse_action(&mut Cursor::new(synchronization(&[bytes]))).is_err());
        }
        let mut payload = synchronization(&[command(&[3, 0, 232, 3, 0, 0, 0x40, 0xff])]);
        payload.pop();
        assert!(parse_action(&mut Cursor::new(payload)).is_err());
        let mut payload = synchronization(&[vec![0]]);
        assert!(parse_action(&mut Cursor::new(payload.clone())).is_err());
        payload[29] ^= 1;
        assert!(parse_action(&mut Cursor::new(payload)).is_err());
    }

    #[test]
    fn record_boundary_prevents_consuming_following_record() {
        let payload = synchronization(&[command(&[3, 0, 232, 3, 0, 0, 0x40, 0xff])]);
        let mut cursor = record(&payload);
        // First record declares one byte less than its command requires.
        cursor.get_mut()[4..8].copy_from_slice(&((payload.len() - 1) as u32).to_le_bytes());
        let len = cursor.get_ref().len() as u64;
        assert!(parse_ticks(&mut cursor, &mut ReplayInfo::default(), len).is_err());
        let mut cursor = record(&payload);
        cursor.get_mut()[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        let len = cursor.get_ref().len() as u64;
        assert!(parse_ticks(&mut cursor, &mut ReplayInfo::default(), len).is_err());
    }

    #[test]
    fn retains_unknown_outer_records() {
        let mut cursor = record(&[9, 8, 7]);
        cursor.get_mut()[..4].copy_from_slice(&99u32.to_le_bytes());
        let len = cursor.get_ref().len() as u64;
        let mut replay = ReplayInfo::default();
        parse_ticks(&mut cursor, &mut replay, len).unwrap();
        assert_eq!(replay.unknown_records.len(), 1);
        assert_eq!(replay.unknown_records[0].record_type, 99);
        assert_eq!(replay.unknown_records[0].payload, vec![9, 8, 7]);
        assert!(replay.unknown_records[0].error.is_none());
    }

    #[test]
    fn retains_structurally_framed_but_unknown_command_layout() {
        // The command length is valid, but custom type 7 declares two bytes
        // while only one is present.  The envelope remains inspectable.
        let body = vec![250, 0, 0, 0, 0, 0, 0x40, 7, 2, 0xaa];
        let payload = synchronization(&[command(&body)]);
        let (actions, _) = parse_action(&mut Cursor::new(payload)).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].data, body);
        assert!(actions[0].command.is_none());
        assert!(actions[0].command_error.is_some());
    }

    #[test]
    fn retains_malformed_chat_as_opaque_record() {
        let mut cursor = record(&[1, 0, 0, 0]);
        cursor.get_mut()[..4].copy_from_slice(&TICK_CHATMSG.to_le_bytes());
        let len = cursor.get_ref().len() as u64;
        let mut replay = ReplayInfo::default();
        parse_ticks(&mut cursor, &mut replay, len).unwrap();
        assert!(replay.messages.is_empty());
        assert_eq!(replay.unknown_records.len(), 1);
        assert!(replay.unknown_records[0].error.is_some());
    }

    #[test]
    fn retains_malformed_sync_as_opaque_record() {
        let mut cursor = record(&[0]);
        let len = cursor.get_ref().len() as u64;
        let mut replay = ReplayInfo::default();
        parse_ticks(&mut cursor, &mut replay, len).unwrap();
        assert!(replay.sync_records.is_empty());
        assert_eq!(replay.unknown_records.len(), 1);
        assert_eq!(replay.unknown_records[0].record_type, TICK_ACTION);
        assert!(replay.unknown_records[0].error.is_some());
    }

    #[test]
    fn retains_chat_wire_fields_and_raw_payload() {
        fn v16(value: &str) -> Vec<u8> {
            let units: Vec<u16> = value.encode_utf16().collect();
            let mut bytes = (units.len() as u32).to_le_bytes().to_vec();
            for unit in units {
                bytes.extend_from_slice(&unit.to_le_bytes());
            }
            bytes
        }

        let mut payload = vec![1, 0, 0, 0, 0, 0, 0, 0];
        payload.extend_from_slice(&v16("Alice"));
        payload.extend_from_slice(&[7, 3, 0, 0]);
        payload.extend_from_slice(&1u32.to_le_bytes());
        payload.extend_from_slice(&1u32.to_le_bytes());
        payload.extend_from_slice(&v16("hello"));
        let size = (payload.len() - 8) as u32;
        payload[4..8].copy_from_slice(&size.to_le_bytes());

        let message = parse_message(&mut Cursor::new(payload.clone()), 42).unwrap();
        assert_eq!(message.wire_kind, 1);
        assert_eq!(message.wire_size, size);
        assert_eq!(message.sender, "Alice");
        assert_eq!(message.player_id, 7);
        assert_eq!(message.player_reserved, [3, 0, 0]);
        assert_eq!(message.kind, 1);
        assert_eq!(message.local, 1);
        assert_eq!(message.raw, payload);
    }

    #[test]
    fn parses_existing_samples() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        for (name, ticks, actions, commands, bundles, messages) in [
            ("3v3.rec", 16779, 256, 16449, 133228, 27),
            ("recs/1.rec", 11760, 355, 2832, 20712, 34),
            ("recs/purchases_SM.rec", 5988, 357, 715, 614, 0),
            ("recs/upgrades.rec", 1530, 24, 211, 185, 0),
            (
                "recs/unit_transformation_and_abilities.rec",
                2332,
                30,
                227,
                194,
                0,
            ),
        ] {
            let replay = parse_replay(&root.join(name)).unwrap();
            assert_eq!(replay.md5.len(), 32, "{} md5", name);
            assert_eq!(replay.ticks, ticks, "{} ticks", name);
            assert_eq!(replay.actions.len(), actions, "{} actions", name);
            assert_eq!(replay.commands.len(), commands, "{} commands", name);
            assert_eq!(
                replay.sync_records.len(),
                ticks as usize,
                "{} sync records",
                name
            );
            assert_eq!(
                replay
                    .sync_records
                    .iter()
                    .map(|record| record.bundles.len())
                    .sum::<usize>(),
                bundles,
                "{} bundles",
                name
            );
            assert!(
                replay
                    .commands
                    .iter()
                    .all(|action| action.command.is_some()),
                "{} commands should decode structurally",
                name
            );
            assert_eq!(replay.messages.len(), messages, "{} messages", name);
        }
    }
}
