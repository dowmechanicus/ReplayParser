use byteorder::{LittleEndian, ReadBytesExt};
use serde::{ser::SerializeStruct, Serialize};
use std::io::{self, Cursor, Error, ErrorKind, Read};

type ActionData<'a> = (&'a Vec<u8>, u32);

/// A receiver reference in an action command.
///
/// `raw` is the decoded wire word. Its high nibble is the receiver namespace
/// and its low 28 bits are the instance ID. The original bytes remain in the
/// action's `data` field because single and list receiver fields use different
/// byte orders.
#[derive(Clone, Debug, Serialize)]
pub struct Receiver {
    pub raw: u32,
    pub kind: u8,
    pub instance_id: u32,
}

impl Receiver {
    fn from_raw(raw: u32) -> Self {
        Self {
            raw,
            kind: (raw >> 28) as u8,
            instance_id: raw & 0x0fff_ffff,
        }
    }
}

/// Decoded fields common to every action command, including unknown opcodes.
///
/// The parser only assigns semantics to framing and fields supported by the
/// inspected serializer. `raw_context` and `custom_data` remain available when
/// the game-specific meaning is unknown.
#[derive(Clone, Debug, Serialize)]
pub struct Command {
    pub opcode: u8,
    pub sender_flags: u8,
    pub sender_slot: u8,
    pub process_type: u8,
    pub raw_context: [u8; 4],
    pub receivers: Vec<Receiver>,
    pub custom_type: u8,
    pub custom_length: Option<u16>,
    pub custom_data: Vec<u8>,
}

impl Command {
    /// Decode the generic command envelope after the inclusive u16 length.
    pub fn decode(data: &[u8]) -> Result<Self, io::Error> {
        if data.len() < 8 {
            return Err(invalid("command body is too short"));
        }

        let opcode = data[0];
        let sender_flags = data[1];
        let sender_slot = sender_flags & 0x7f;
        let process_type = sender_flags >> 7;
        let raw_context = [data[2], data[3], data[4], data[5]];
        let mut cursor = Cursor::new(&data[6..]);
        let receivers = read_receivers(&mut cursor)?;
        let custom_type = cursor.read_u8()?;
        let (custom_length, custom_data) = if custom_type == 0xff {
            if cursor.position() != cursor.get_ref().len() as u64 {
                return Err(invalid("custom type 0xff has payload bytes"));
            }
            (None, Vec::new())
        } else {
            let high = cursor.read_u8()?;
            let length = if high < 0x80 {
                high as u16
            } else {
                let low = cursor.read_u8()?;
                (((high & 0x7f) as u16) << 8) | low as u16
            };
            let remaining = cursor.get_ref().len() as u64 - cursor.position();
            if remaining != length as u64 {
                return Err(invalid("custom payload length mismatch"));
            }
            let mut bytes = vec![0; length as usize];
            cursor.read_exact(&mut bytes)?;
            (Some(length), bytes)
        };

        Ok(Self {
            opcode,
            sender_flags,
            sender_slot,
            process_type,
            raw_context,
            receivers,
            custom_type,
            custom_length,
            custom_data,
        })
    }
}

fn invalid(message: &'static str) -> io::Error {
    Error::new(ErrorKind::InvalidData, message)
}

fn read_receivers(cursor: &mut Cursor<&[u8]>) -> Result<Vec<Receiver>, io::Error> {
    let first = cursor.read_u8()?;
    if first & 0xc0 == 0 {
        let mut bytes = [first, 0, 0, 0];
        cursor.read_exact(&mut bytes[1..])?;
        return Ok(vec![Receiver::from_raw(u32::from_be_bytes(bytes))]);
    }

    let count = match first & 0xc0 {
        0x40 => (first & 0x3f) as u32,
        0x80 => (u16::from_be_bytes([first, cursor.read_u8()?]) & 0x3fff) as u32,
        0xc0 => {
            let mut bytes = [first, 0, 0, 0];
            cursor.read_exact(&mut bytes[1..])?;
            u32::from_be_bytes(bytes) & 0x3fff_ffff
        }
        _ => unreachable!(),
    };
    let remaining = cursor.get_ref().len() as u64 - cursor.position();
    if count as u64 > remaining / 4 {
        return Err(invalid("receiver list exceeds command body"));
    }
    let mut receivers = Vec::with_capacity(count as usize);
    for _ in 0..count {
        receivers.push(Receiver::from_raw(cursor.read_u32::<LittleEndian>()?));
    }
    Ok(receivers)
}

#[derive(Clone, Debug)]
pub struct Action {
    pub player: String,
    pub relic_id: u64,
    pub tick: u32,
    /// Complete command body, starting with the opcode; excludes the u16 wire length.
    pub data: Vec<u8>,
    /// Structured generic fields, present for commands accepted by the decoder.
    pub command: Option<Command>,
    /// If the command envelope is structurally framed but has an unknown
    /// layout, retain the raw body and expose the decoder failure here rather
    /// than dropping the command or aborting the replay.
    pub command_error: Option<String>,
}

impl<'a> From<ActionData<'a>> for Action {
    fn from(action_data: ActionData<'a>) -> Self {
        let (data, tick) = action_data;

        match Command::decode(data) {
            Ok(command) => Self {
                player: String::new(),
                relic_id: 0,
                tick,
                data: data.clone(),
                command: Some(command),
                command_error: None,
            },
            Err(error) => Self::from_raw(data.clone(), tick, error.to_string()),
        }
    }
}

impl Action {
    pub fn from_decoded(data: Vec<u8>, tick: u32, command: Command) -> Self {
        Self {
            player: String::new(),
            relic_id: 0,
            tick,
            data,
            command: Some(command),
            command_error: None,
        }
    }

    pub fn from_raw(data: Vec<u8>, tick: u32, error: String) -> Self {
        Self {
            player: String::new(),
            relic_id: 0,
            tick,
            data,
            command: None,
            command_error: Some(error),
        }
    }
}

impl Serialize for Action {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Action", 6)?;
        state.serialize_field("relic_id", &self.relic_id)?;
        state.serialize_field("name", &self.player)?;
        state.serialize_field("tick", &self.tick)?;
        state.serialize_field("data", &self.data)?;
        state.serialize_field("command", &self.command)?;
        state.serialize_field("command_error", &self.command_error)?;
        state.end()
    }
}
