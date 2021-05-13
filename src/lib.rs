#![no_std]

use core::convert::TryFrom;

use embedded_hal::serial::{Read, Write};
use heapless as h;
use packet::*;

pub(crate) mod packet;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq)]
/// An OSCP device address. The controller should always have address 1. Peripheral devices
/// can have any other non-zero address. Address 0 is used to denote a broadcast message.
pub struct Addr(u8);
pub enum Command {
    Reset,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Ready,

    Waiting,
}

/// Maximum packet length.
/// Currently set to 19 bytes, which is the 3-byte header, and then a maximum of 16 data bytes.
pub const MAX_LENGTH: usize = 19;

pub struct Oscp<READ, WRITE> {
    read: READ,
    write: WRITE,
    address: Addr,
    state: State,
    read_buffer: h::Vec<u8, MAX_LENGTH>,
}

pub enum Error {
    NotReady,
    IoError,
    ParseError,
    General,
}

pub type Result<T> = core::result::Result<T, Error>;

impl<READ, WRITE> Oscp<READ, WRITE> {
    pub fn peripheral(read: READ, write: WRITE, address: Addr) -> Self
    where
        READ: Read<u8>,
        WRITE: Write<u8>,
    {
        Self {
            read,
            write,
            address,
            state: State::Waiting,
            read_buffer: h::Vec::new(),
        }
    }

    pub fn controller(read: READ, write: WRITE) -> Self
    where
        READ: Read<u8>,
        WRITE: Write<u8>,
    {
        Self {
            read,
            write,
            address: Addr(1),
            state: State::Ready,
            read_buffer: h::Vec::new(),
        }
    }
}

impl<READ, WRITE> Oscp<READ, WRITE>
where
    READ: Read<u8>,
    WRITE: Write<u8>,
{
    pub fn send_command(&mut self, dest: Addr, _cmd: Command) -> Result<()> {
        let packet = Packet {
            dest,
            packet_type: Type::Command,
        };
        self.send_packet(&packet)
    }

    /// Send a generic `Packet`. No error checking is done on whether the packet is valid,
    /// however it should be (close to) impossible for a user to create an invalid packet.
    pub fn send_packet(&mut self, packet: &Packet) -> Result<()> {
        if self.state != State::Ready {
            Err(Error::NotReady)
        } else {
            packet.write(&mut self.write)?;
            Ok(())
        }
    }

    /// Poll the input stream for more data, and return a `Packet` if a full packet has been
    /// read and is available.
    pub fn poll(&mut self) -> Result<Option<Packet>> {
        loop {
            let byte: nb::Result<u8, _> = self.read.read();
            match byte {
                Err(nb::Error::WouldBlock) => return Ok(None),
                Err(nb::Error::Other(_)) => {
                    // An IoError has occurred, reset the buffer and return the error
                    self.reset_read_buffer();
                    return Err(Error::IoError);
                }
                Ok(b) => {
                    if let Err(_) = self.read_buffer.push(b) {
                        panic!("Maximum packet length exceeded");
                    }
                    if let Some(packet) = self.parse_packet()? {
                        self.reset_read_buffer();
                        return Ok(Some(packet));
                    } // else continue loop               }
                }
            }
        }
    }

    fn parse_packet(&self) -> Result<Option<Packet>> {
        if self.read_buffer.len() < 3 {
            Ok(None)
        } else {
            let addr = self.read_buffer[0];
            let packet_type = self.read_buffer[1];
            let len = self.read_buffer[2].into();
            if self.read_buffer.len() - 3 < len {
                Ok(None) // packet not yet complete.
            } else {
                self.decode_packet(
                    Addr(addr),
                    packet::Type::try_from(packet_type)?,
                    len,
                    &self.read_buffer[3..],
                )
            }
        }
    }

    fn decode_packet(
        &self,
        addr: Addr,
        typ: packet::Type,
        len: usize,
        data: &[u8],
    ) -> Result<Option<Packet>> {
        todo!()
    }

    fn reset_read_buffer(&mut self) {
        self.read_buffer.clear();
    }
}
