use protocol::*;
use protocol_derive::*;
use std::collections::BTreeMap;
use crate::peers::{PeerID, KeyID};

#[derive(Protocol, Debug, Clone)]
pub enum Packet {
    // Networking packets
    ProtocolVersion(ProtocolVersionPacket),
    InitRsa(InitRsaPacket),
    InitAes(InitAesPacket),
    Ehlo(EhloPacket),

    // Peer discovery
    DiscoverPeers(DiscoverPeersPacket),
    DiscoverPeersResp(DiscoverPeersRespPacket),

    // Kademlia DHT
    FindDhtValue(FindDhtValuePacket),
    FindDhtValueResp(FindDhtValueRespPacket),
    FindPeer(FindPeerPacket),
    FindPeerResp(FindPeerRespPacket),
    StoreDhtValue(StoreDhtValuePacket),

    // Utility packets
    Ping(PingPacket),
    Pong(PingPacket),
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
#[derive(Protocol, Debug, Clone)]
pub struct EhloPacket {
    /// The address you want peers to connect to
    pub addr: String,
}

/// Sent by a node to discover peers in a bucket.
/// Peers will be selected if `peer_id & mask == target & mask`.
#[derive(Protocol, Debug, Clone)]
pub struct DiscoverPeersPacket {
    /// Unique request id used to match the response to the request.
    pub request_id: u32,
    /// Not necessarly an existing peer, more like a prefix to select peers in the same bucket.
    pub target: PeerID,
    /// max-size: 64 bytes
    pub mask: Vec<u8>,
    pub limit: u16,
}

/// Response to [DiscoverPeersPacket].
/// In contrast to [FindPeer], this response shouldn't contain highly trusted data.  
/// The priority here is to prevent two nodes from sharing too many connections, in order to strenghten the network.  
/// Hence, it is better to reply with nodes we are not connected to.
#[derive(Protocol, Debug, Clone)]
pub struct DiscoverPeersRespPacket {
    /// Request id from the [DiscoverPeersPacket] packet.
    pub request_id: u32,
    /// Peers that match the request.  
    /// Ordered by preference determined by the responder.
    /// The requester is urged to follow these recommandations.
    /// 
    /// Note: if [DiscoverPeersPacket::limit] is reached, the replier should still add highly trusted peers at the end of the list, to ensure that not all results are poor quality.
    pub peers: Vec<(PeerID, String)>,
}

/// *Request for [`FindDhtValueRespPacket`]*
#[derive(Protocol, Debug, Clone)]
pub struct FindDhtValuePacket {
    /// Unique request id used to match the response to the request.
    pub request_id: u32,
    /// The key to find.
    pub key: KeyID,
    /// Maximum number of peers to return if the key is not found.
    pub limit_peers: u16,
    /// Maximum number of values to return if the key is found.
    pub limit_values: u16,
}

/// *Response to [`FindDhtValuePacket`]*
#[derive(Protocol, Debug, Clone)]
pub struct FindDhtValueRespPacket {
    /// Unique request id used to match the response to the request.
    pub request_id: u32,
    pub result: DhtLookupResult,
}

#[derive(Protocol, Debug, Clone)]
pub enum DhtLookupResult {
    /// The value was found.
    Found(Vec<crate::node::DhtValue>),
    /// The value was not found, but here are some peers that might have it.
    /// Same particularities as [FindPeerRespPacket::peers]
    NotFound(Vec<(PeerID, String)>),
}

/// *Request for [`FindPeerRespPacket`]*
#[derive(Protocol, Debug, Clone)]
pub struct FindPeerPacket {
    /// Unique request id used to match the response to the request.
    pub request_id: u32,
    /// The peer to find.
    pub peer_id: PeerID,
    /// Maximum number of peers to return.
    pub limit: u16,
}

/// *Response to [`FindPeerPacket`]*
#[derive(Protocol, Debug, Clone)]
pub struct FindPeerRespPacket {
    /// Unique request id used to match the response to the request.
    pub request_id: u32,
    /// Peers that could be useful for further completion of the request.
    /// This list might contain the peer we are looking for.
    /// The order does not matter as this will probably be reordered by the receiver of this packet.
    pub peers: Vec<(PeerID, String)>,
}

/// TODO: Kademlia store value
#[derive(Protocol, Debug, Clone)]
pub struct StoreDhtValuePacket {

}


#[derive(Protocol, Debug, Clone, Copy)]
pub struct PingPacket {
    pub ping_id: u32,
}

/// Sent by a disconnecting node.
/// All packets sent or received after this one is potentially ignored.
#[derive(Protocol, Debug, Clone)]
pub struct QuitPacket {
    pub code: String,
    // TODO [#19]: Add message in `Quit` packet
    // pub message: String,
}
