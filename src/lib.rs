#![no_std]

use core::{
    convert::{Infallible, TryFrom},
    hash::Hasher,
};

use bitflags::bitflags;
use crc::crc32;
use embedded_hal::serial::{Read, Write};
use heapless as h;

pub enum Error {
    IoError,
    ParseError,
}

/// Packet containing data of type `D`. In general, D should implement Encode and Decode
pub struct Packet<D> {
    typ: PacketType,
    flags: Flags,
    target: Addr,
    data: D,
}

bitflags! {
    struct Flags: u8 {
        const IGNORE = 0b00000001;
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Addr(u8);

pub const CONTROLLER: Addr = Addr(0);
pub const BROADCAST: Addr = Addr(255);

pub type CRC = u32; // For now.

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum PacketType {
    Command = 0x01,
    MidiEvent = 0x02,

    Raw = 0xFF, // not really useful on its own
}

impl TryFrom<u8> for PacketType {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(PacketType::Command),
            0x02 => Ok(PacketType::MidiEvent),
            0xFF => Ok(PacketType::Raw),
            _ => Err(Error::ParseError),
        }
    }
}

/// Each packet contains 32 data bytes.
const PACKET_LEN: usize = 32;

type Raw = h::Vec<u8, PACKET_LEN>;

pub trait Encode {
    fn data(&self) -> Raw;
}

pub trait Decode: Sized {
    type Error;
    fn decode(raw: Raw) -> Result<Self, Self::Error>;
}

impl Encode for Raw {
    fn data(&self) -> Raw {
        self.clone()
    }
}

impl Decode for Raw {
    type Error = Infallible;
    fn decode(raw: Raw) -> Result<Self, Self::Error> {
        Ok(raw)
    }
}

impl<D> Packet<D>
where
    D: Encode + Decode,
{
    pub fn write(&self, s: impl Write<u8>) -> Result<(), Error> {
        self.encoded().write_raw(s)
    }

    pub fn with_data<F>(&self, data: F) -> Packet<F> {
        Packet {
            typ: self.typ,
            flags: self.flags,
            target: self.target,
            data,
        }
    }

    pub fn encoded(&self) -> Packet<Raw> {
        let d = self.data.data();
        self.with_data(d)
    }
}

impl Packet<Raw> {
    /// Write out a raw packet to the stream
    pub fn write_raw(&mut self, s: impl Write<u8>) -> Result<(), Error> {
        let mut out = DigesterOutput::new(s);
        out.write(self.typ as u8)?;
        out.write(self.flags.bits())?;
        out.write(self.target.0)?;
        out.write_data(self.data.as_ref())?;
        out.write_checksum()?;

        Ok(())
    }

    /// Read in a raw packet
    pub fn read_raw(s: impl Read<u8>) -> Result<Self, Error> {
        let mut input = DigesterInput::new(s);
        let packet_type = PacketType::try_from(input.read()?)?;
        let flags = Flags::from_bits(input.read()?).ok_or(Error::ParseError)?;
        let target = Addr(input.read()?);
        let mut data: [u8; PACKET_LEN] = Default::default();
        input.read_data(&mut data)?;
        input.read_checksum()?;
        let data = h::Vec::from_slice(&data).map_err(|_| Error::ParseError)?;

        Ok(Packet {
            typ: packet_type,
            flags,
            target,
            data,
        })
    }
}

/// Writes data to a serial device, and calculates the CRC32 checksum as data is written
struct DigesterOutput<O> {
    output: O,
    digest: crc32::Digest,
}

impl<O: Write<u8>> DigesterOutput<O> {
    fn new(output: O) -> Self {
        let digest = crc32::Digest::new(crc32::IEEE);
        Self { output, digest }
    }

    /// Write a single byte
    fn write(&mut self, d: u8) -> Result<(), Error> {
        self.output.write(d).map_err(to_io_error)?;
        self.digest.write_u8(d);
        Ok(())
    }

    /// Write a number of bytes from a buffer.
    fn write_data(&mut self, d: &[u8]) -> Result<(), Error> {
        for b in d {
            self.write(*b)?;
        }
        Ok(())
    }

    /// Write out the calculated checksum, and return it.
    fn write_checksum(&mut self) -> Result<CRC, Error> {
        let digest = self.digest.finish() as u32;
        for b in &digest.to_le_bytes() {
            self.output.write(*b).map_err(to_io_error)?;
        }
        Ok(digest)
    }
}

/// Reads data from a serial device, and cumulatively calculates the CRC32 checksum
struct DigesterInput<I> {
    input: I,
    digest: crc32::Digest,
}

impl<I: Read<u8>> DigesterInput<I> {
    fn new(input: I) -> Self {
        let digest = crc32::Digest::new(crc32::IEEE);
        Self { input, digest }
    }

    /// Read a single byte
    fn read(&mut self) -> Result<u8, Error> {
        let d = self.input.read().map_err(to_io_error)?;
        self.digest.write_u8(d);

        Ok(d)
    }

    /// Read `LEN` bytes into a buffer
    fn read_data<const LEN: usize>(&mut self, buf: &mut [u8; LEN]) -> Result<(), Error> {
        for i in 0..LEN {
            buf[i] = self.read()?;
        }
        Ok(())
    }

    /// Read the checksum from the stream, and compare it to the calculated checksum
    fn read_checksum(&mut self) -> Result<(), Error> {
        let mut buf: [u8; 4] = Default::default();
        for i in 0..4 {
            buf[i] = self.input.read().map_err(to_io_error)?;
        }
        let packet_checksum = u32::from_le_bytes(buf);
        let calc_checksum = self.digest.finish() as u32;
        if packet_checksum == calc_checksum {
            Ok(())
        } else {
            Err(Error::IoError)
        }
    }
}

fn to_io_error<E>(_err: nb::Error<E>) -> Error {
    Error::IoError // TODO: return context info along with error
}
