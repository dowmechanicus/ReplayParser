use std::io::{self, Cursor, Read, Seek, SeekFrom, Error, ErrorKind};
use byteorder::{ReadBytesExt, LittleEndian};

#[derive(Default,Serialize)]
pub struct Game { 
    pub name: String,
    pub mode: String,
    pub resources: String,
    pub locations: String,
    pub victory_points: u32,
}
#[derive(Serialize)]
pub struct Data { pub duration: u32 }
#[derive(Default,Serialize)]
pub struct Map { 
    pub name: String,
    pub description: String,
    pub abbrname: String,
    pub maxplayers: u32,
    pub path: String,
    pub date: String,
    pub width: u32,
    pub height: u32,
}
#[derive(Default,Serialize)]
pub struct Player {
    pub name: String,
    pub kind: u32,
    pub team: u32,
    pub race: String,
    pub relic_id: u64,
    pub rank: u32,
    pub cpu: u32,
    pub hero: u32,
    pub skin_path: String,
    pub skin_name: String,
    pub id: u8,
}
#[derive(Serialize)]
#[serde(untagged)]
pub enum Chunk {
    Game(Game),
    Data(Data),
    Map(Map),
    Player(Player),
    FoldInfo { size: u32 },
    Empty {}
}

impl Default for Chunk {
    fn default() -> Chunk { Chunk::Empty {}}
}

pub fn parse(mut cursor: &mut Cursor<Vec<u8>>) -> Result<Chunk, io::Error> {
    let mut buf = vec![0; 8];
    cursor.read_exact(&mut buf)?;
    let chunk_name = String::from_utf8(buf).unwrap_or("".to_string());
    let _chunk_version = cursor.read_u32::<LittleEndian>()?;
    let chunk_size = cursor.read_u32::<LittleEndian>()?;

    /*
    println!("chunk {} ver={} size={}",
             chunk_name, chunk_version, chunk_size);
    */

    cursor.seek(SeekFrom::Current(12))?;

    let result = match chunk_name.as_ref() {
        "DATADATA" => parse_data(&mut cursor),
        "DATASDSC" => parse_sdsc(&mut cursor),
        "DATABASE" => parse_base(&mut cursor),
        "DATAINFO" => parse_info(&mut cursor),
        "FOLDINFO" => Ok(Chunk::FoldInfo { size: chunk_size }),
        "FOLDPOST" | "FOLDGPLY" => Ok(Chunk::Empty {}),
        _ => {
            let msg = format!("invalid chunk \"{}\" at position {}",
                              chunk_name, cursor.position());
            return Err(Error::new(ErrorKind::InvalidData, msg))
        }
    }?;

    Ok(result)
}

fn parse_data(cursor: &mut Cursor<Vec<u8>>) -> Result<Chunk, io::Error> {
    Ok(Chunk::Data(Data {
        duration: cursor.read_u32::<LittleEndian>()?
    }))
}

fn parse_sdsc(mut cursor: &mut Cursor<Vec<u8>>) -> Result<Chunk, io::Error> {
    cursor.seek(SeekFrom::Current(4))?;
    let date = read_vstring_utf16(&mut cursor); 
    cursor.seek(SeekFrom::Current(8))?;
    let path = read_vstring(&mut cursor);
    let name = read_vstring_utf16(&mut cursor); 
    let abbrname = read_vstring_utf16(&mut cursor); 
    let description = read_vstring_utf16(&mut cursor); 
    let maxplayers = cursor.read_u32::<LittleEndian>().unwrap();
    let width = cursor.read_u32::<LittleEndian>().unwrap();
    let height = cursor.read_u32::<LittleEndian>().unwrap();

    cursor.seek(SeekFrom::Current(12 + 4 + path.len() as i64))?;

    Ok(Chunk::Map(Map {
        date: date,
        path: path,
        name: name,
        abbrname: abbrname,
        description: description,
        maxplayers: maxplayers,
        width: width,
        height: height,
    }))
}

fn parse_base(mut cursor: &mut Cursor<Vec<u8>>) -> Result<Chunk, io::Error> {
    let mut vpc = 0;
    let mut resources = String::new();
    let mut locations = String::new();
    let mut mode = "Annihilation".to_owned();
    cursor.seek(SeekFrom::Current(12))?;
    let nparams = cursor.read_u32::<LittleEndian>()?;
    for _ in 0..nparams {
        let val = cursor.read_u32::<LittleEndian>()?;
        let mut buf = vec![0; 4];
        cursor.read_exact(&mut buf)?;
        let key = String::from_utf8(buf).unwrap_or("".to_string());
        match key.as_ref() {
            "KTPV" => vpc = 250 * 2u32.pow(val),
            "TSSR" if val == 0 => resources = "Standard".to_owned(),
            "TSSR" if val != 0 => resources = "High".to_owned(),
            "COLS" if val == 0 => locations = "Random".to_owned(),
            "COLS" if val != 0 => locations = "Fixed".to_owned(),
            _ => ()
        };
    }

    cursor.seek(SeekFrom::Current(1))?;
    let name = read_vstring_utf16(&mut cursor); 
    cursor.seek(SeekFrom::Current(4))?;
    let nconds = cursor.read_u32::<LittleEndian>()?;
    for _ in 0..nconds {
        let i = cursor.read_u32::<LittleEndian>()?;
        if i == 0x1576eb00 {
            mode = "Victory Point Control".to_owned();
        }
    }
    cursor.seek(SeekFrom::Current(12))?;

    Ok(Chunk::Game(Game {
        name: name,
        mode: mode,
        resources: resources,
        locations: locations,
        victory_points: vpc,
    }))
}

fn parse_info(mut cursor: &mut Cursor<Vec<u8>>) -> Result<Chunk, io::Error> {
    let name = read_vstring_utf16(&mut cursor);
    let kind = cursor.read_u32::<LittleEndian>()?;
    let team = cursor.read_u32::<LittleEndian>()?;
    let race = read_vstring(&mut cursor);
    let relic_id = cursor.read_u64::<LittleEndian>()?;
    let rank = cursor.read_u32::<LittleEndian>()?;
    cursor.seek(SeekFrom::Current(4))?;
    let cpu = cursor.read_u32::<LittleEndian>()?;
    let hero = cursor.read_u32::<LittleEndian>()?;
    cursor.seek(SeekFrom::Current(10))?;
    let skin_path = read_vstring(&mut cursor);
    cursor.seek(SeekFrom::Current(4))?;
    let skin_name = read_vstring_utf16(&mut cursor);
    let id = cursor.read_u8()?;
    let tmp = cursor.read_u8()?;
    if tmp == 0 || tmp == 0xff {
        cursor.seek(SeekFrom::Current(2))?;
    }

    Ok(Chunk::Player(Player {
        name: name,
        kind: kind,
        team: team,
        race: race,
        relic_id: relic_id,
        rank: rank,
        cpu: cpu,
        hero: hero,
        skin_path: skin_path,
        skin_name: skin_name,
        id: id
    }))
}

pub fn read_vstring(cursor: &mut Cursor<Vec<u8>>) -> String {
    let nchars = cursor.read_u32::<LittleEndian>().unwrap_or(0);
    let mut buf = vec![0; nchars as usize];

    if let Err(_) = cursor.read_exact(&mut buf) {
        return "".to_string()
    }

    String::from_utf8(buf).unwrap_or("".to_string())
}

pub fn read_vstring_utf16(cursor: &mut Cursor<Vec<u8>>) -> String {
    let nchars = cursor.read_u32::<LittleEndian>().unwrap_or(0);
    let mut buf: Vec<u16> = Vec::with_capacity(nchars as usize);

    for _ in 0..nchars {
        match cursor.read_u16::<LittleEndian>() {
            Ok(c) => buf.push(c),
            Err(_) => (),
        }
    }

    String::from_utf16(&buf).unwrap_or("".to_string())
}
