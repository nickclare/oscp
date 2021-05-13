use core::convert::TryFrom;

use super::{Addr, Error, Result};
use embedded_hal::serial::Write;
pub enum Type {
    MidiEvent,
    Command,
}

pub struct Packet {
    pub dest: Addr,
    pub packet_type: Type,
}

impl<X> From<nb::Error<X>> for Error {
    fn from(_: nb::Error<X>) -> Self {
        Error::IoError
    }
}

impl Packet {
    pub fn write<W: Write<u8>>(&self, out: &mut W) -> Result<()> {
        self.write_addr(out)?;
        self.write_packet_type(out)?;
        Ok(())
    }

    fn write_addr<W: Write<u8>>(&self, out: &mut W) -> Result<()> {
        out.write(self.dest.0)?;
        Ok(())
    }

    fn write_packet_type<W: Write<u8>>(&self, out: &mut W) -> Result<()> {
        out.write(self.packet_type.into())?;
        Ok(())
    }
}

impl TryFrom<u8> for Type {
    type Error = super::Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0x01 => Ok(Type::MidiEvent),
            0x10 => Ok(Type::Command),
            _ => Err(Error::ParseError),
        }
    }
}

impl Into<u8> for Type {
    fn into(self) -> u8 {
        match self {
            Type::MidiEvent => 0x01,
            Type::Command => 0x10,
        }
    }
}
