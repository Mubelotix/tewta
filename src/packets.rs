use protocol::*;
use protocol_derive::*;

#[derive(Protocol, Debug, Clone)]
pub enum Packet {
    Ping(PingPacket),
    Pong(PingPacket),
}

#[derive(Protocol, Debug, Clone, Copy)]
pub struct PingPacket {
    pub ping_id: u32,
}
