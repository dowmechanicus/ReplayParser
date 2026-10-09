use byteorder::{LittleEndian, ReadBytesExt};
use crypto::digest::Digest;
use crypto::md5::Md5;

use chunky::Chunk;
use chunky::Data as DataChunk;
use chunky::Player as PlayerChunk;

use crate::message::Message;
use crate::replay::ReplayInfo;

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
    let len = bytes.len() as u64;
    let mut cursor = Cursor::new(bytes);

    let version = cursor.read_u32::<LittleEndian>()?;
    let mod_chksum = cursor.read_u32::<LittleEndian>()?;
    cursor.seek(SeekFrom::Current(4))?;
    cursor.seek(SeekFrom::Current(8))?;

    let mut buf: Vec<u16> = Vec::new();
    for _ in 0..19 {
        let c = cursor.read_u16::<LittleEndian>().unwrap_or(0);
        if c > 31 && c < 123 {
            buf.push(c);
        }
    }

    let mut replay = ReplayInfo {
        mod_chksum,
        mod_version: version,
        date: String::from_utf16(&buf).unwrap(),
        ..Default::default()
    };

    cursor.seek(SeekFrom::Current(26))?;
    let mut buf = vec![0; 12];
    cursor.read_exact(&mut buf)?;
    //let file_format = String::from_utf8(buf).unwrap_or("".to_string());

    cursor.seek(SeekFrom::Current(24))?;

    parse_chunks(&mut cursor, &mut replay, len)?;
    parse_ticks(&mut cursor, &mut replay, len)?;

    match_player_ids_from_messages(&mut replay);

    Ok(replay)
}

pub fn parse_chunks(
    cursor: &mut Cursor<Vec<u8>>,
    replay: &mut ReplayInfo,
    pos: u64,
) -> Result<(), io::Error> {
    chunky::parse(cursor)?;
    if let Chunk::Data(DataChunk { duration }) = chunky::parse(cursor)? {
        replay.ticks = duration;
    }
    cursor.seek(SeekFrom::Current(36))?;

    let mut endpos = pos;
    loop {
        if cursor.position() >= endpos {
            break; // end of header chunks, start of actions
        }

        match chunky::parse(cursor)? {
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
    }

    Ok(())
}
pub fn parse_ticks(
    cursor: &mut Cursor<Vec<u8>>,
    replay: &mut ReplayInfo,
    pos: u64,
) -> Result<(), io::Error> {
    let mut current_tick = 0;
    let mut md5 = Md5::new();

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
                let (actions, tick) = parse_action(&mut payload)?;

                if tick > 0 {
                    current_tick = tick
                }

                if !actions.is_empty() {
                    for action in actions {
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
                            replay.actions.push(action);
                        }
                    }
                }
            }
            TICK_CHATMSG => {
                let msg = parse_message(&mut payload, current_tick)?;
                replay.messages.push(msg);
            }
            _ => return Err(Error::new(ErrorKind::InvalidData, "invalid action")),
        };
        if payload.position() != tick_size {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "record payload length mismatch",
            ));
        }
    }

    replay.md5 = md5.result_str();

    Ok(())
}

/// Read a synchronization record payload (the outer type/length are already removed).
/// Bundles contain a u64 metadata value, u32 byte length, one redundant low length
/// byte, then commands framed by a little-endian u16 length including that u16.
pub fn parse_action(cursor: &mut Cursor<Vec<u8>>) -> Result<(Vec<Action>, u32), io::Error> {
    cursor.read_u8()?; // synchronization message tag (0x20 in inspected files)
    let tick = cursor.read_u32::<LittleEndian>()?;
    cursor.read_u32::<LittleEndian>()?;
    cursor.read_u32::<LittleEndian>()?;
    let nbundles = cursor.read_u32::<LittleEndian>()?;
    let mut actions = Vec::new();

    for _ in 0..nbundles {
        cursor.read_u64::<LittleEndian>()?; // preserve interpretation as unknown metadata
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
            actions.push(Action::from((&data, tick)));
        }
    }
    Ok((actions, tick))
}

pub fn parse_message(cursor: &mut Cursor<Vec<u8>>, tick: u32) -> Result<Message, io::Error> {
    // Skip this data for now (we do not YET know what it contains)
    cursor.seek(SeekFrom::Current(8))?;

    // Derive the players name from the next chunk of data
    let sender = chunky::read_vstring_utf16(cursor);

    // Derive the player id from the next chunk of data
    let player_id = cursor.read_u8()?;

    cursor.seek(SeekFrom::Current(3))?;

    let kind = cursor.read_u32::<LittleEndian>()?;
    let local = cursor.read_u32::<LittleEndian>()?;
    let body = chunky::read_vstring_utf16(cursor);

    let receiver = match local {
        1 if kind == 1 => "observers".to_string(),
        1 if kind != 1 => "team".to_string(),
        _ => "all".to_string(),
    };

    Ok(Message {
        tick,
        sender,
        receiver,
        body,
        player_id,
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

    for action in replay.actions.iter_mut() {
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
        let json = serde_json::to_value(&actions[0]).unwrap();
        assert_eq!(json["data"], serde_json::json!(purchase));
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
    fn parses_existing_samples() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        for (name, ticks, actions, messages) in [
            ("3v3.rec", 16779, 256, 27),
            ("recs/1.rec", 11760, 355, 34),
            ("recs/purchases_SM.rec", 5988, 357, 0),
            ("recs/upgrades.rec", 1530, 24, 0),
            ("recs/unit_transformation_and_abilities.rec", 2332, 30, 0),
        ] {
            let replay = parse_replay(&root.join(name)).unwrap();
            assert_eq!(replay.ticks, ticks, "{} ticks", name);
            assert_eq!(replay.actions.len(), actions, "{} actions", name);
            assert_eq!(replay.messages.len(), messages, "{} messages", name);
        }
    }
}
