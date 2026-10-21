use std::fmt;

#[derive(Debug, Clone)]
pub struct MidiFile {
    pub division: u16,
    pub tracks: Vec<MidiTrack>,
    pub tempos: Vec<TempoEvent>,
}

#[derive(Debug, Clone)]
pub struct MidiTrack {
    pub events: Vec<MidiEvent>,
}

#[derive(Debug, Clone, Copy)]
pub struct TempoEvent {
    pub tick: u64,
    pub micros_per_quarter: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct MidiEvent {
    pub tick: u64,
    pub message: MidiMessage,
}

#[derive(Debug, Clone, Copy)]
pub enum MidiMessage {
    NoteOff { channel: u8, note: u8 },
    NoteOn { channel: u8, note: u8, velocity: u8 },
    Other,
}

#[derive(Debug, Clone)]
pub struct MidiError {
    message: String,
}

impl MidiError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for MidiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}

impl std::error::Error for MidiError {}

pub type MidiResult<T> = Result<T, MidiError>;

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }

    fn read_u8(&mut self) -> MidiResult<u8> {
        let byte = *self
            .bytes
            .get(self.pos)
            .ok_or_else(|| MidiError::new("unexpected end of MIDI data"))?;
        self.pos += 1;
        Ok(byte)
    }

    fn read_exact(&mut self, len: usize) -> MidiResult<&'a [u8]> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or_else(|| MidiError::new("MIDI chunk length overflow"))?;
        let bytes = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| MidiError::new("unexpected end of MIDI data"))?;
        self.pos = end;
        Ok(bytes)
    }

    fn read_u16(&mut self) -> MidiResult<u16> {
        let bytes = self.read_exact(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self) -> MidiResult<u32> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_vlq(&mut self) -> MidiResult<u32> {
        let mut value = 0u32;

        for _ in 0..4 {
            let byte = self.read_u8()?;
            value = (value << 7) | u32::from(byte & 0x7F);
            if (byte & 0x80) == 0 {
                return Ok(value);
            }
        }

        Err(MidiError::new("invalid MIDI variable-length quantity"))
    }
}

impl MidiFile {
    pub fn parse(bytes: &[u8]) -> MidiResult<Self> {
        let mut cursor = Cursor::new(bytes);
        if cursor.read_exact(4)? != b"MThd" {
            return Err(MidiError::new("missing MIDI header chunk"));
        }

        let header_len = cursor.read_u32()? as usize;
        if header_len < 6 {
            return Err(MidiError::new("MIDI header chunk is too short"));
        }

        let format = cursor.read_u16()?;
        if format > 1 {
            return Err(MidiError::new(format!(
                "unsupported MIDI format {format}; only format 0 and 1 are supported"
            )));
        }

        let track_count = cursor.read_u16()? as usize;
        let division = cursor.read_u16()?;
        if (division & 0x8000) != 0 {
            return Err(MidiError::new(
                "SMPTE time-division MIDI files are not supported",
            ));
        }
        if division == 0 {
            return Err(MidiError::new("MIDI ticks-per-quarter division is zero"));
        }

        if header_len > 6 {
            cursor.read_exact(header_len - 6)?;
        }

        let mut tracks = Vec::with_capacity(track_count);
        let mut tempos = Vec::new();

        for _ in 0..track_count {
            if cursor.read_exact(4)? != b"MTrk" {
                return Err(MidiError::new("missing MIDI track chunk"));
            }

            let len = cursor.read_u32()? as usize;
            let track_bytes = cursor.read_exact(len)?;
            let track = parse_track(track_bytes, &mut tempos)?;
            tracks.push(track);
        }

        tempos.sort_by_key(|tempo| tempo.tick);
        if tempos.first().is_none_or(|tempo| tempo.tick != 0) {
            tempos.insert(
                0,
                TempoEvent {
                    tick: 0,
                    micros_per_quarter: 500_000,
                },
            );
        }

        Ok(Self {
            division,
            tracks,
            tempos,
        })
    }
}

fn parse_track(bytes: &[u8], tempos: &mut Vec<TempoEvent>) -> MidiResult<MidiTrack> {
    let mut cursor = Cursor::new(bytes);
    let mut tick = 0u64;
    let mut running_status: Option<u8> = None;
    let mut events = Vec::new();

    while cursor.remaining() > 0 {
        let delta = cursor.read_vlq()?;
        tick += u64::from(delta);

        let first = cursor.read_u8()?;
        let status = if first & 0x80 != 0 {
            running_status = Some(first);
            first
        } else {
            let status = running_status
                .ok_or_else(|| MidiError::new("MIDI running status used before status byte"))?;
            cursor.pos = cursor.pos.saturating_sub(1);
            status
        };

        match status {
            0xFF => {
                running_status = None;
                let meta_type = cursor.read_u8()?;
                let len = cursor.read_vlq()? as usize;
                let data = cursor.read_exact(len)?;
                if meta_type == 0x51 && data.len() == 3 {
                    tempos.push(TempoEvent {
                        tick,
                        micros_per_quarter: u32::from(data[0]) << 16
                            | u32::from(data[1]) << 8
                            | u32::from(data[2]),
                    });
                }
                if meta_type == 0x2F {
                    break;
                }
            }
            0xF0 | 0xF7 => {
                running_status = None;
                let len = cursor.read_vlq()? as usize;
                cursor.read_exact(len)?;
            }
            0x80..=0xEF => {
                let high = status >> 4;
                let channel = status & 0x0F;
                let data_len = if high == 0x0C || high == 0x0D { 1 } else { 2 };
                let data = cursor.read_exact(data_len)?;
                let message = match high {
                    0x08 => MidiMessage::NoteOff {
                        channel,
                        note: data[0],
                    },
                    0x09 => MidiMessage::NoteOn {
                        channel,
                        note: data[0],
                        velocity: data[1],
                    },
                    _ => MidiMessage::Other,
                };
                events.push(MidiEvent { tick, message });
            }
            _ => {
                return Err(MidiError::new(format!(
                    "unsupported MIDI status byte 0x{status:02X}"
                )));
            }
        }
    }

    Ok(MidiTrack { events })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_format_zero() {
        let midi = [
            b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 0, 0, 1, 0, 96, b'M', b'T', b'r', b'k', 0, 0, 0,
            11, 0, 0x90, 60, 100, 96, 60, 0, 0, 0xFF, 0x2F, 0,
        ];

        let parsed = MidiFile::parse(&midi).unwrap();

        assert_eq!(parsed.division, 96);
        assert_eq!(parsed.tracks.len(), 1);
        assert_eq!(parsed.tracks[0].events.len(), 2);
    }

    #[test]
    fn test_rejects_smpte_division() {
        let midi = [b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 0, 0, 0, 0xE7, 0x28];

        assert!(MidiFile::parse(&midi).is_err());
    }
}
