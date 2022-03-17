use protocol::*;
use protocol_derive::*;
use std::collections::BTreeMap;
use crate::peers::PeerID;

#[derive(Protocol, Debug, Clone)]
pub enum Packet {
    ProtocolVersion(ProtocolVersionPacket),
    InitRsa(InitRsaPacket),
    InitAes(InitAesPacket),
    Ehlo(EhloPacket),
    Ping(PingPacket),
    Pong(PingPacket),
    RequestPeerMap,
    PeerMap(PeerMapPacket),
    Quit(QuitPacket),
}

/// The protocol version packet.
/// This is the first packet ever sent by clients.
/// 
/// It MUST never change.
#[derive(Protocol, Debug, Clone)]
pub struct ProtocolVersionPacket {
    /// Constant, equal to "p2pnet"
    pub protocol: String,
    /// The supported [semver](https://semver.org/) versions of the protocol.
    /// Nodes will select the highest commonly supported version.
    pub supported_versions: Vec<(u32, u32, u32)>,
}

/// First part of the handshake.
/// Should be sent by both peers unencrypted right after the protocol version packet.  
/// 
/// The public key is used to encrypt the next packet ([`InitAesPacket`]).
/// It is also used to get the peer id: `peer_id = sha512(exponent + modulus)` where exponent and modulus bytes are reprensented as little endian.
#[derive(Protocol, Debug, Clone)]
pub struct InitRsaPacket {
    /// Little endian exponent of the public key.
    pub rsa_public_key_exponent: Vec<u8>,
    /// Little endian modulus of the public key.
    pub rsa_public_key_modulus: Vec<u8>,
    /// A 16 bytes nonce to send back in [`InitAesPacket`]
    pub nonce: Vec<u8>,
}

/// Last part of the handshake.
/// Can also be sent at any time to reset the AES encryption.
///
/// Encrypted with the recipient public key.
/// All future messages will be encrypted with AES.
#[derive(Protocol, Debug, Clone)]
pub struct InitAesPacket {
    /// 16 bytes used to encrypt all future messages.  
    /// This is only one part of the AES key as both sides generate a half.
    /// When concatenating, the peer with the lowest PeerId puts its part fitst.
    pub aes_key_part: Vec<u8>,
    /// A clone of the nonce from [`InitRsaPacket`]
    pub nonce: Vec<u8>,
}

/// Initialize data between nodes.
/// Sent by both peers right after the encryption handshake.
#[derive(Protocol, Debug, Clone, Copy)]
pub struct EhloPacket {

}

#[derive(Protocol, Debug, Clone, Copy)]
pub struct PingPacket {
    pub ping_id: u32,
}

/// Sent by a node on request.
/// Contains the list of its connected peers.
/// The node might also recommend arbitrarly a few peers to try to connect to, without guaranteeing their availability.
#[derive(Protocol, Debug, Clone)]
pub struct PeerMapPacket {
    // Todo: Replace peer address with more appropriate type
    pub peers: BTreeMap<PeerID, String>,
    pub recommended_peers: BTreeMap<PeerID, String>,
}

/// Sent by a disconnecting node.
/// All packets sent or received after this one is potentially ignored.
#[derive(Protocol, Debug, Clone)]
pub struct QuitPacket {
    pub code: String,
    // TODO [#19]: Add message in `Quit` packet
    // pub message: String,
}
