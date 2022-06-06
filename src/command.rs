use anyhow::anyhow;
use anyhow::Error;
use anyhow::Result;
use byteorder::BigEndian;
use byteorder::WriteBytesExt;
use std::io::Cursor;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use byteorder::ReadBytesExt;

pub const VERSION: u8 = 0;

type Version = u8;
type PlayerHash = u64;

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum Status {
    OK,
    ERR,
    VersionMismatch,
}

#[repr(u8)]
pub enum CommandMap {
    Response,
    Handshake,
    PlaceTile,
    UpdateTile,
    Disconnect,
    Handshaken,
}

impl TryFrom<u8> for CommandMap {
    type Error = Error;

    fn try_from(num: u8) -> Result<CommandMap> {
        match num {
            0 => Ok(CommandMap::Response),
            1 => Ok(CommandMap::Handshake),
            2 => Ok(CommandMap::PlaceTile),
            3 => Ok(CommandMap::UpdateTile),
            4 => Ok(CommandMap::Disconnect),
            _ => {
                println!("Unsupported command code '{}' prodided", num);
                Err(anyhow!("Unsupported command code"))
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Command {
    Response(Status),
    Handshake(Version, PlayerHash),
    PlaceTile(i32, i32, i32, i32),
    UpdateTile(u8, i32, i32, i32, i32),
    Disconnect,
    Handshaken(Status, u8),
}

impl Command {
    pub fn into_bytes(&self, buf: &mut Vec<u8>) -> usize {
        let mut buf = Cursor::new(buf);
        match self {
            Command::Response(status) => {
                WriteBytesExt::write_u8(&mut buf, CommandMap::Response as u8).unwrap();
                WriteBytesExt::write_u8(&mut buf, *status as u8).unwrap();
                return buf.position() as usize;
            }
            Command::UpdateTile(id, region, x, y, z) => {
                WriteBytesExt::write_u8(&mut buf, CommandMap::UpdateTile as u8).unwrap();
                WriteBytesExt::write_u8(&mut buf, *id).unwrap();
                WriteBytesExt::write_i32::<BigEndian>(&mut buf, *region).unwrap();
                WriteBytesExt::write_i32::<BigEndian>(&mut buf, *x).unwrap();
                WriteBytesExt::write_i32::<BigEndian>(&mut buf, *y).unwrap();
                WriteBytesExt::write_i32::<BigEndian>(&mut buf, *z).unwrap();
                return buf.position() as usize;
            },
            Command::Handshaken(status, id) => {
                WriteBytesExt::write_u8(&mut buf, CommandMap::Handshaken as u8).unwrap();
                WriteBytesExt::write_u8(&mut buf, *status as u8).unwrap();
                WriteBytesExt::write_u8(&mut buf, *id).unwrap();
                return buf.position() as usize;
            }
            _ => unimplemented!(),
        }
    }

    pub async fn send(&self, buf: &mut Vec<u8>, socket: &mut TcpStream) -> Result<()> {
        let n = self.into_bytes(buf);
        socket.write_all(&buf[0..n]).await?;
        Ok(())
    }
}

impl TryFrom<&mut Cursor<&[u8]>> for Command {
    type Error = Error;

    fn try_from(buf: &mut Cursor<&[u8]>) -> Result<Command> {
        let map: CommandMap = buf.read_u8()?.try_into()?;

        match map {
            CommandMap::Handshake => Ok(Command::Handshake(
                buf.read_u8()?,
                buf.read_u64::<BigEndian>()?,
            )),
            CommandMap::PlaceTile => Ok(Command::PlaceTile(
                buf.read_i32::<BigEndian>()?,
                buf.read_i32::<BigEndian>()?,
                buf.read_i32::<BigEndian>()?,
                buf.read_i32::<BigEndian>()?,
            )),
            CommandMap::Disconnect => Ok(Command::Disconnect),
            _ => Err(anyhow!("Server code sent from client!")),
        }
    }
}
