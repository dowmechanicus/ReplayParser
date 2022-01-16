use std::env;
use std::fs::File;
use std::path::Path;
use std::io::{self, Read, Seek, SeekFrom, Cursor, Error, ErrorKind};

extern crate byteorder;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate crypto;

use byteorder::{ReadBytesExt, LittleEndian};
use crypto::md5::Md5;
use crypto::digest::Digest;

mod chunky;
use chunky::Chunk;
use chunky::Data as DataChunk;
use chunky::Player as PlayerChunk;

const TICK_ACTION: u32 = 0;
const TICK_CHATMSG: u32 = 1;

#[derive(Serialize)]
pub struct Action {
    tick: u32,
    data: Vec<u8>,
}
#[derive(Serialize)]
pub struct Message {
    tick: u32,
    sender: String,
    receiver: String,
    body: String,
}

#[derive(Default,Serialize)]
pub struct ReplayInfo {
    name: String,
    mod_chksum: u32,
    mod_version: u32,
    md5: String,
    date: String,
    ticks: u32,
    game: Chunk,
    map: Chunk,
    players: Vec<Chunk>,
    observers: Vec<Chunk>,
    messages: Vec<Message>,
}

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

fn parse_replay(path: &Path) -> Result<ReplayInfo, io::Error> {
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

fn parse_chunks(mut cursor: &mut Cursor<Vec<u8>>,
                mut replay: &mut ReplayInfo,
                pos: u64) -> Result<(), io::Error> {
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
               Chunk::Empty { .. }      => (),
               Chunk::FoldInfo { size } => endpos = cursor.position() +
                                                    size as u64,
               Chunk::Data(DataChunk { duration }) => replay.ticks = duration,
            c@ Chunk::Map { .. }        => replay.map = c,
            c@ Chunk::Game { .. }       => replay.game = c,
            c@ Chunk::Player { .. }     => { 
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
fn parse_ticks(mut cursor: &mut Cursor<Vec<u8>>,
               mut replay: &mut ReplayInfo,
               pos: u64) -> Result<(), io::Error> {
    let mut current_tick = 0;
    let mut md5 = Md5::new();

    loop {
        if cursor.position() >= pos {
            break;
        }

        let tick_type = cursor.read_u32::<LittleEndian>()?;
        let _tick_size = cursor.read_u32::<LittleEndian>()?;

        match tick_type {
            TICK_ACTION  => {
                let action = parse_action(&mut cursor)?;
                if action.tick > 0 { current_tick = action.tick }
                md5.input(action.data.as_slice());
            }
            TICK_CHATMSG => {
                let msg = parse_message(&mut cursor, current_tick)?;
                replay.messages.push(msg);
            }
            _ => return Err(Error::new(ErrorKind::InvalidData, "invalid action"))
        };
    }

    replay.md5 = md5.result_str();
    println!("{}", replay.md5);

    Ok(())
}

fn parse_action(cursor: &mut Cursor<Vec<u8>>) -> Result<Action, io::Error> {
    cursor.seek(SeekFrom::Current(1))?;
    let tick = cursor.read_u32::<LittleEndian>()?;
    cursor.seek(SeekFrom::Current(8))?;
    let mut actions = Vec::new();
    let nactions = cursor.read_u32::<LittleEndian>()?;
    for _ in 0..nactions {
        cursor.seek(SeekFrom::Current(8))?;
        let mut bytes_remain = cursor.read_u32::<LittleEndian>()?;
        while bytes_remain > 0 {
            cursor.seek(SeekFrom::Current(1))?;
            let action_size = cursor.read_u8()?;
            let mut buf = vec![0; (action_size-2) as usize];
            cursor.read_exact(&mut buf)?;
            actions.append(&mut buf);
            bytes_remain -= action_size as u32;
        }
        cursor.seek(SeekFrom::Current(1))?;
    }

    // println!("tick: {}, actions: {:?}", tick, actions);

    Ok(Action {
        tick: tick,
        data: actions,
    })
}

fn parse_message(mut cursor: &mut Cursor<Vec<u8>>, tick: u32)
                 -> Result<Message, io::Error> {
    cursor.seek(SeekFrom::Current(8))?;
    let sender = chunky::read_vstring_utf16(&mut cursor); 
    cursor.seek(SeekFrom::Current(4))?;
    let kind = cursor.read_u32::<LittleEndian>()?;
    let local = cursor.read_u32::<LittleEndian>()?;
    let body = chunky::read_vstring_utf16(&mut cursor); 

    let receiver = match local {
        1 if kind == 1 => "observers".to_string(),
        1 if kind != 1 => "team".to_string(),
        _ => "all".to_string()
    };

    Ok(Message {
        tick: tick,
        sender: sender,
        receiver: receiver,
        body: body,
    })
}

fn main() {
    if let Some(arg) = env::args().nth(1) {
        let path = Path::new(&arg);
        let replay = match parse_replay(&path) {
            Ok(r) => r,
            Err(e) => {
                println!("error: {}", e);
                std::process::exit(1);
            }
        };
        let json = serde_json::to_string_pretty(&replay).unwrap();
        println!("{}", json);

    } else {
        println!("must supply a file");
    }
}

