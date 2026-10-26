use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{self, Cursor, Error, ErrorKind, Read, Seek, SeekFrom};

#[derive(Clone, Debug, Default, Serialize)]
pub struct Game {
    pub name: String,
    /// Name of the first win condition, or "None" if the list is empty.
    pub mode: String,
    pub resources: String,
    pub locations: String,
    pub victory_points: u32,
    pub random_seed: u32,
    /// Every win-condition ID stored in the replay, in file order.
    pub win_conditions: Vec<WinCondition>,
}
/// One entry of the `DATABASE` win-condition list.
#[derive(Clone, Debug, Default, Serialize)]
pub struct WinCondition {
    /// CRC32 of the win-condition `.scar` file's contents.
    pub id: u32,
    pub name: String,
}
#[derive(Debug, Serialize)]
pub struct Data {
    pub duration: u32,
}
#[derive(Clone, Debug, Default, Serialize)]
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
#[derive(Debug, Default, Serialize)]
pub struct Player {
    pub name: String,
    pub kind: u32,
    pub team: u32,
    pub race: String,
    pub relic_id: u64,
    pub rank: u32,
    pub cpu: u32,
    pub hero: u32,
    pub primary_color: u8,
    pub secondary_color: u8,
    pub trim_color: u8,
    pub accessory_color: u8,
    pub skin_path: String,
    pub skin_name: String,
    pub id: u8,
    /// Position of this player's DATAINFO among all lobby blocks, empty ones included.
    pub lobby_block: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player_id: Option<u32>,
}
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Chunk {
    Game(Game),
    Data(Data),
    Map(Map),
    Player(Player),
    FoldInfo { size: u32 },
    Empty {},
}

impl Default for Chunk {
    fn default() -> Chunk {
        Chunk::Empty {}
    }
}

pub fn parse(cursor: &mut Cursor<Vec<u8>>) -> Result<Chunk, io::Error> {
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
        "DATADATA" => parse_data(cursor),
        "DATASDSC" => parse_sdsc(cursor),
        "DATABASE" => parse_base(cursor),
        "DATAINFO" => parse_info(cursor),
        "FOLDINFO" => Ok(Chunk::FoldInfo { size: chunk_size }),
        "FOLDPOST" | "FOLDGPLY" => Ok(Chunk::Empty {}),
        _ => {
            let msg = format!(
                "invalid chunk \"{}\" at position {}",
                chunk_name,
                cursor.position()
            );
            return Err(Error::new(ErrorKind::InvalidData, msg));
        }
    }?;

    Ok(result)
}

fn parse_data(cursor: &mut Cursor<Vec<u8>>) -> Result<Chunk, io::Error> {
    Ok(Chunk::Data(Data {
        duration: cursor.read_u32::<LittleEndian>()?,
    }))
}

fn parse_sdsc(cursor: &mut Cursor<Vec<u8>>) -> Result<Chunk, io::Error> {
    cursor.seek(SeekFrom::Current(4))?;
    let date = read_vstring_utf16(cursor);
    cursor.seek(SeekFrom::Current(8))?;
    let path = read_vstring(cursor);
    let name = read_vstring_utf16(cursor);
    let abbrname = read_vstring_utf16(cursor);
    let description = read_vstring_utf16(cursor);
    let maxplayers = cursor.read_u32::<LittleEndian>().unwrap();
    let width = cursor.read_u32::<LittleEndian>().unwrap();
    let height = cursor.read_u32::<LittleEndian>().unwrap();

    cursor.seek(SeekFrom::Current(12 + 4 + path.len() as i64))?;

    Ok(Chunk::Map(Map {
        date,
        path,
        name,
        abbrname,
        description,
        maxplayers,
        width,
        height,
    }))
}

/// Display name for a win-condition ID from the `DATABASE` chunk. The game
/// stores the CRC32 of the win-condition `.scar` file's contents, so the ID
/// depends on the game data or mod that was loaded (see
/// docs/replay-format.md, "Win conditions").
fn win_condition_name(id: u32) -> String {
    let name = match id {
        // Stock Retribution data (000_data.sga)
        0x5286_c903 => "Annihilation",
        0xa5d2_fc11 => "Free For All (solo)",
        0x0575_0182 => "Free For All (team)",
        0x1576_eb00 => "Victory Point Control",
        // Elite mod (Elite_Data_18_Misc.sga)
        0x69b1_8aa9 => "Annihilation",
        0xc156_f468 => "Free For All (solo)",
        0x776c_a03b => "Free For All (team)",
        0xfc03_e252 => "Victory Point Control",
        0xc843_8f73 => "Annihilate - Hero Assault",
        0xe034_f94e => "Massive Battles - Annihilate",
        0x1aaf_d5c8 => "VPC - Hero Assault (500 points)",
        0x978d_d4ce => "Massive Battles - VPC (750 points)",
        0xa049_9908 => "[VPC] Sandbox Mode",
        0xba07_9905 => "[Event] Night Fighting",
        0xe51d_3657 => "Tabletop Mode",
        _ => return format!("Unknown win condition 0x{id:08x}"),
    };
    name.to_owned()
}

fn parse_base(cursor: &mut Cursor<Vec<u8>>) -> Result<Chunk, io::Error> {
    let mut vpc = 0;
    let mut resources = String::new();
    let mut locations = String::new();
    let mut win_conditions: Vec<WinCondition> = Vec::new();
    // Word 3 is the game's random seed.
    cursor.seek(SeekFrom::Current(8))?;
    let random_seed = cursor.read_u32::<LittleEndian>()?;
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
            _ => (),
        };
    }

    cursor.seek(SeekFrom::Current(1))?;
    let name = read_vstring_utf16(cursor);
    cursor.seek(SeekFrom::Current(4))?;
    let nconds = cursor.read_u32::<LittleEndian>()?;
    for _ in 0..nconds {
        let id = cursor.read_u32::<LittleEndian>()?;
        win_conditions.push(WinCondition {
            id,
            name: win_condition_name(id),
        });
    }
    let mode = win_conditions
        .first()
        .map_or_else(|| "None".to_owned(), |condition| condition.name.clone());
    cursor.seek(SeekFrom::Current(12))?;

    Ok(Chunk::Game(Game {
        name,
        mode,
        resources,
        locations,
        victory_points: vpc,
        random_seed,
        win_conditions,
    }))
}

fn parse_info(cursor: &mut Cursor<Vec<u8>>) -> Result<Chunk, io::Error> {
    let name = read_vstring_utf16(cursor);
    let kind = cursor.read_u32::<LittleEndian>()?;
    let team = cursor.read_u32::<LittleEndian>()?;
    let race = read_vstring(cursor);
    let relic_id = cursor.read_u64::<LittleEndian>()?;
    let rank = cursor.read_u32::<LittleEndian>()?;

    // These seem to be empty -> 0
    cursor.seek(SeekFrom::Current(4))?;

    let cpu = cursor.read_u32::<LittleEndian>()?;
    let hero = cursor.read_u32::<LittleEndian>()?;

    cursor.seek(SeekFrom::Current(1))?;

    // Rank again
    cursor.seek(SeekFrom::Current(4))?;
    // Primary
    let primary_color = cursor.read_u8()?;
    // Secondary
    let secondary_color = cursor.read_u8()?;
    // Trim
    let trim_color = cursor.read_u8()?;
    // Accessory
    let accessory_color = cursor.read_u8()?;

    cursor.seek(SeekFrom::Current(1))?;

    let skin_path = read_vstring(cursor);

    // These seem to be empty. Probably padding for the strings.
    cursor.seek(SeekFrom::Current(4))?;

    let skin_name = read_vstring_utf16(cursor);
    let id = cursor.read_u8()?;
    let tmp = cursor.read_u8()?;
    if tmp == 0 || tmp == 0xff {
        cursor.seek(SeekFrom::Current(2))?;
    }

    Ok(Chunk::Player(Player {
        name,
        kind,
        team,
        race,
        relic_id,
        rank,
        cpu,
        hero,
        primary_color,
        secondary_color,
        trim_color,
        accessory_color,
        skin_path,
        skin_name,
        id,
        lobby_block: 0,
        slot: None,
        player_id: None,
    }))
}

pub fn read_vstring(cursor: &mut Cursor<Vec<u8>>) -> String {
    let nchars = cursor.read_u32::<LittleEndian>().unwrap_or(0);
    let mut buf = vec![0; nchars as usize];

    if let Err(_) = cursor.read_exact(&mut buf) {
        return "".to_string();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_random_seed_from_database_chunk() {
        let mut bytes = Vec::new();
        for word in [1u32, 2, 0x5622_3305] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes.extend_from_slice(&1u32.to_le_bytes()); // nparams
        bytes.extend_from_slice(&0u32.to_le_bytes()); // COLS value
        bytes.extend_from_slice(b"COLS");
        bytes.push(0);
        bytes.extend_from_slice(&0u32.to_le_bytes()); // empty name
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&0u32.to_le_bytes()); // nconds
        bytes.extend_from_slice(&[0; 12]);

        match parse_base(&mut Cursor::new(bytes)).unwrap() {
            Chunk::Game(game) => {
                assert_eq!(game.random_seed, 0x5622_3305);
                assert_eq!(game.locations, "Random");
            }
            _ => panic!("expected a game chunk"),
        }
    }

    fn base_chunk_with_conditions(ids: &[u32]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for word in [1u32, 2, 3] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes.extend_from_slice(&0u32.to_le_bytes()); // nparams
        bytes.push(0);
        bytes.extend_from_slice(&0u32.to_le_bytes()); // empty name
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&(ids.len() as u32).to_le_bytes()); // nconds
        for id in ids {
            bytes.extend_from_slice(&id.to_le_bytes());
        }
        bytes.extend_from_slice(&[0; 12]);
        bytes
    }

    fn mode_of(ids: &[u32]) -> String {
        match parse_base(&mut Cursor::new(base_chunk_with_conditions(ids))).unwrap() {
            Chunk::Game(game) => game.mode,
            _ => panic!("expected a game chunk"),
        }
    }

    #[test]
    fn names_stock_and_elite_victory_point_control() {
        assert_eq!(mode_of(&[0x1576_eb00]), "Victory Point Control");
        assert_eq!(mode_of(&[0xfc03_e252]), "Victory Point Control");
    }

    #[test]
    fn reports_unknown_and_multiple_win_conditions() {
        assert_eq!(mode_of(&[0x1234_5678]), "Unknown win condition 0x12345678");
        assert_eq!(mode_of(&[]), "None");

        let bytes = base_chunk_with_conditions(&[0x5286_c903, 0xe51d_3657]);
        match parse_base(&mut Cursor::new(bytes)).unwrap() {
            Chunk::Game(game) => {
                assert_eq!(game.mode, "Annihilation");
                let listed: Vec<(u32, &str)> = game
                    .win_conditions
                    .iter()
                    .map(|condition| (condition.id, condition.name.as_str()))
                    .collect();
                assert_eq!(
                    listed,
                    vec![
                        (0x5286_c903, "Annihilation"),
                        (0xe51d_3657, "Tabletop Mode")
                    ]
                );
            }
            _ => panic!("expected a game chunk"),
        }
    }
}
