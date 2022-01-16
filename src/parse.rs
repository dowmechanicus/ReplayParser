use byteorder::{LittleEndian, ReadBytesExt};
use crypto::digest::Digest;
use crypto::md5::Md5;

use chunky::Chunk;
use chunky::Data as DataChunk;
use chunky::Player as PlayerChunk;

use crate::message::Message;
use crate::replay::ReplayInfo;

use std::fs::File;
use std::io::{self, Cursor, Error, ErrorKind, Read, Seek, SeekFrom};

use std::path::Path;

use crate::actions::{Action, ActionType};
use crate::chunky;

const TICK_ACTION: u32 = 0;
const TICK_CHATMSG: u32 = 1;

fn read_rec_file(path: &Path) -> Result<Vec<u8>, io::Error> {
    let mut file = File::open(path)?;
    let mut buf = [0; 20];
    let mut vec = Vec::new();

    file.read(&mut buf)?;

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
        mod_chksum: mod_chksum,
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

    Ok(replay)
}

pub fn parse_chunks(
    mut cursor: &mut Cursor<Vec<u8>>,
    mut replay: &mut ReplayInfo,
    pos: u64,
) -> Result<(), io::Error> {
    chunky::parse(&mut cursor)?;
    if let Chunk::Data(DataChunk { duration }) = chunky::parse(&mut cursor)? {
        replay.ticks = duration;
    }
    cursor.seek(SeekFrom::Current(36))?;

    let mut endpos = pos;
    loop {
        if cursor.position() >= endpos {
            break; // end of header chunks, start of actions
        }

        match chunky::parse(&mut cursor)? {
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
    mut cursor: &mut Cursor<Vec<u8>>,
    mut replay: &mut ReplayInfo,
    pos: u64,
) -> Result<(), io::Error> {
    let mut current_tick = 0;
    let mut md5 = Md5::new();

    loop {
        if cursor.position() >= pos {
            break;
        }

        let tick_type = cursor.read_u32::<LittleEndian>()?;
        let _tick_size = cursor.read_u32::<LittleEndian>()?;

        match tick_type {
            TICK_ACTION => {
                let (actions, tick) = parse_action(&mut cursor)?;

                if tick > 0 {
                    current_tick = tick
                }

                if actions.len() > 0 {
                    for mut action in actions {
                        // Retrieve the players name from the players vector and
                        // associate it with the action.
                        if let Chunk::Player(player) = &replay.players[action.player_id as usize] {
                            action.player_name = player.name.to_string();
                        };

                        match &action.details {
                            ActionType::BuildUnit(..) => replay.actions.push(action),
                            ActionType::CancelUnitOrWargear(..) => replay.actions.push(action),
                            ActionType::UpgradeBuilding(..) => replay.actions.push(action),
                            ActionType::UpgradeUnit(..) => replay.actions.push(action),
                            ActionType::PurchaseWargear(..) => replay.actions.push(action),
                            _ => (),
                        };
                    }
                }
            }
            TICK_CHATMSG => {
                let msg = parse_message(&mut cursor, current_tick)?;
                replay.messages.push(msg);
            }
            _ => return Err(Error::new(ErrorKind::InvalidData, "invalid action")),
        };
    }

    replay.md5 = md5.result_str();

    Ok(())
}

/// Reads the data from an action tick
///
/// * DWORD [4 bytes] = Identifies this as an Action Tick
/// * DWORD [4 bytes] = Length in bytes of the remainder of this action tick
/// * ------- This is where the cursor starts from in this function -------
/// * BYTE [1 byte] = Always the same
/// * DWORD [4 bytes] = The number of the current tick. Starts with 1
/// * DWORD [4 bytes] = Another counter, but does not go up with every tick. Seems somehow related to the chainging of action tick lengths. Start with 0.
/// * DWORD [4 bytes] = Unknown. Varies with every action tick.
/// * DWORD [4 bytes] = The amount of player actions bundles in this tick.
///
pub fn parse_action(cursor: &mut Cursor<Vec<u8>>) -> Result<(Vec<Action>, u32), io::Error> {
    // Always the same
    cursor.seek(SeekFrom::Current(1))?;

    // The number of the current tick
    let tick = cursor.read_u32::<LittleEndian>()?;

    // Reading another counter and the unknown field...
    let meta = vec![
        cursor.read_u32::<LittleEndian>()?,
        cursor.read_u32::<LittleEndian>()?,
    ];

    let mut action_bundle = vec![];

    // ...and continue with the amount player actions bundles in this tick
    let nactions = cursor.read_u32::<LittleEndian>()?;

    for _ in 0..nactions {
        // Player Action Bundle Shared Header
        // We don't know what the next to DWORDs contain, so we skip them
        cursor.seek(SeekFrom::Current(8))?;

        // The remaining size of the action bundle + 1
        let mut bytes_remain = cursor.read_u32::<LittleEndian>()?;
        // Header End

        while bytes_remain > 0 {
            // Total length of player actions in bytes.
            // DO NOT USE THIS VALUE AS THE REMAINING LENGTH (it is limited to 1 byte).
            cursor.seek(SeekFrom::Current(1))?;

            // Length of the next action block in bytes included this byte
            let action_size = cursor.read_u8()?;

            let mut buf = vec![0; (action_size - 2) as usize];
            cursor.read_exact(&mut buf)?;

            let mut action = Action::from((&buf, tick));

            action.meta = meta.clone();

            action_bundle.push(action);

            bytes_remain -= action_size as u32;
        }
        cursor.seek(SeekFrom::Current(1))?;
    }

    Ok((action_bundle, tick))
}

pub fn parse_message(mut cursor: &mut Cursor<Vec<u8>>, tick: u32) -> Result<Message, io::Error> {
    cursor.seek(SeekFrom::Current(8))?;
    let sender = chunky::read_vstring_utf16(&mut cursor);
    cursor.seek(SeekFrom::Current(4))?;
    let kind = cursor.read_u32::<LittleEndian>()?;
    let local = cursor.read_u32::<LittleEndian>()?;
    let body = chunky::read_vstring_utf16(&mut cursor);

    let receiver = match local {
        1 if kind == 1 => "observers".to_string(),
        1 if kind != 1 => "team".to_string(),
        _ => "all".to_string(),
    };

    Ok(Message {
        tick: tick,
        sender: sender,
        receiver: receiver,
        body: body,
    })
}
