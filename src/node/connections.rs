use super::*;

#[cfg(not(feature = "test"))]
pub type ReadHalf = tokio::net::tcp::OwnedReadHalf;
#[cfg(not(feature = "test"))]
pub type WriteHalf = tokio::net::tcp::OwnedWriteHalf;
#[cfg(feature = "test")]
pub type ReadHalf = crate::stream::testing::TestReadHalf;
#[cfg(feature = "test")]
pub type WriteHalf = crate::stream::testing::TestWriteHalf;

struct Peer {
    /// How we connected to that peer. Useful for peer routing.
    addr: String,
    write_stream: WriteHalf,
    ping_nanos: Option<usize>,
}

pub(super) struct ConnectionPool {
    connections: Mutex<BTreeMap<PeerID, Peer>>,
    node_ref: UnsafeCell<Weak<Node>>,
}

unsafe impl Sync for ConnectionPool {}

impl ConnectionPool {
    pub(super) fn set_node_ref(&self, node_ref: Weak<Node>) {
        unsafe {
            *self.node_ref.get() = node_ref;
        }
    }

    pub(super) async fn send_packet(&self, n: &PeerID, p: Packet) {
        // TODO [#26]: Remove these ugly weak upgraded refs
        let node = Weak::clone(unsafe {&*self.node_ref.get()}).upgrade().unwrap();

        let p = match p.raw_bytes(&PROTOCOL_SETTINGS) {
            Ok(p) => p,
            Err(e) => {
                error!(node.log_level, "{:?}", e);
                return;
            }
        };

        let mut connections = self.connections.lock().await;

        let peer = match connections.get_mut(n) {
            Some(s) => s,
            None => {
                warn!(node.log_level, "no connection to {}", n);
                return;
            },
        };

        // Write packet prefixed with length
        let len = p.len() as u32;
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&len.to_be_bytes());
        peer.write_stream.write_all(&buf).await.unwrap();
        peer.write_stream.write_all(&p).await.unwrap();
        trace!(node.log_level, "packet written to {}: {:?}", n, p);
    }

    pub(super) async fn set_ping(&self, n: &PeerID, ping_nanos: usize) {
        let node = Weak::clone(unsafe {&*self.node_ref.get()}).upgrade().unwrap();
        let mut connections = self.connections.lock().await;
        match connections.get_mut(n) {
            Some(p) => p.ping_nanos = Some(ping_nanos),
            None => warn!(node.log_level, "unable to set ping: no connection to {}", n),
        };
    }

    pub(super) async fn disconnect(&self, n: &PeerID) {
        let mut connections = self.connections.lock().await;
        // TODO [#20]: Warn on debug if node is already disconnected
        connections.remove(n);

        // TODO [#21]: Send quit packet when disconnecting
        // We will have to add a parameter in this function
    }

    pub(super) async fn insert(&self, n: PeerID, mut s: TcpStream, addr: String) {
        let mut connections = self.connections.lock().await;
        let (mut read_stream, write_stream) = s.into_split();
        let peer = Peer {
            addr,
            write_stream,
            ping_nanos: None,
        };
        connections.insert(n.clone(), peer);
        let node = Weak::clone(unsafe {&*self.node_ref.get()});

        // Listen for messages from the remote node
        tokio::spawn(async move {
            loop {
                // TODO [#22]: Aes encryption
                // For receiving and sending

                // Read packet
                let packet_size = read_stream.read_u32().await.unwrap();
                if packet_size >= MAX_PACKET_SIZE {
                    warn!(node.upgrade().unwrap().log_level, "packet size too large");
                    unimplemented!("Recovery of packet size too large");
                }
                let mut packet = Vec::with_capacity(packet_size as usize);
                unsafe {packet.set_len(packet_size as usize)};
                read_stream.read_exact(&mut packet).await.unwrap();

                // Parse packet
                let packet: Packet = match Parcel::from_raw_bytes(&packet, &PROTOCOL_SETTINGS) {
                    Ok(p) => p,
                    Err(e) => {
                        warn!(node.upgrade().unwrap().log_level, "Failed to parse packet {:?}", e);
                        continue;
                    },
                };

                // Handle packet
                // Warning: This blocks the packet receiving loop.
                node.upgrade().unwrap().on_packet(n.clone(), packet).await;
            }
        });
    }

    pub(super) async fn len(&self) -> usize {
        let connections = self.connections.lock().await;
        connections.len()
    }

    pub(super) async fn prepare_find_peers_response(&self, n: &PeerID, p: FindPeersPacket) -> ReturnPeersPacket {
        let connections = self.connections.lock().await;
        let mut peers = Vec::new();
        for (peer_id, peer) in connections.iter() {
            peers.push((peer_id.clone(), peer.addr.clone()));
        }
        std::mem::drop(connections); // Release the lock

        // TODO [#23]: Avoid returning the peer that makes the request when certain conditions are met

        peers.sort_by_key(|(id, _)| id.distance(&p.target));
        peers.truncate(std::cmp::min(MAX_PEERS_RETURNED, p.limit) as usize);

        ReturnPeersPacket {
            request_id: p.request_id,
            peers,
        }
    }

    /// Returns a list of all connected node IDs
    pub(super) async fn connected_nodes(&self) -> Vec<PeerID> {
        let connections = self.connections.lock().await;
        connections.keys().cloned().collect()
    }
}

impl Default for ConnectionPool {
    fn default() -> ConnectionPool {
        ConnectionPool {
            connections: Mutex::new(BTreeMap::new()),
            node_ref: UnsafeCell::new(Weak::new()),
        }
    }
}
