use super::*;

#[cfg(not(feature = "test"))]
pub type ReadHalf = tokio::net::tcp::OwnedReadHalf;
#[cfg(not(feature = "test"))]
pub type WriteHalf = tokio::net::tcp::OwnedWriteHalf;
#[cfg(feature = "test")]
pub type ReadHalf = crate::stream::testing::TestReadHalf;
#[cfg(feature = "test")]
pub type WriteHalf = crate::stream::testing::TestWriteHalf;

struct PeerInfo {
    /// How we connected to that peer. Useful for peer routing.
    addr: String,
    write_stream: WriteHalf,
    ping_nanos: Option<usize>,

    // Todo: Hold reputation data here in the PeerInfo struct
}
pub(super) struct ConnectionPool {
    connections: Mutex<BTreeMap<PeerID, PeerInfo>>,
    our_peer_id: PeerID,
    ll: LogLevel,
    node_ref: UnsafeCell<Weak<Node>>,
}

unsafe impl Sync for ConnectionPool {}

impl ConnectionPool {
    pub fn new(our_peer_id: PeerID, ll: LogLevel) -> ConnectionPool {
        ConnectionPool {
            connections: Mutex::new(BTreeMap::new()),
            our_peer_id,
            ll,
            node_ref: UnsafeCell::new(Weak::new()),
        }
    }

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
                error!(node.ll, "{:?}", e);
                return;
            }
        };

        let mut connections = self.connections.lock().await;

        let peer = match connections.get_mut(n) {
            Some(s) => s,
            None => {
                warn!(node.ll, "no connection to {}", n);
                return;
            },
        };

        // Write packet prefixed with length
        let len = p.len() as u32;
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&len.to_be_bytes());
        peer.write_stream.write_all(&buf).await.unwrap();
        peer.write_stream.write_all(&p).await.unwrap();
        trace!(node.ll, "packet written to {}: {:?}", n, p);
    }

    pub(super) async fn set_ping(&self, n: &PeerID, ping_nanos: usize) {
        let node = Weak::clone(unsafe {&*self.node_ref.get()}).upgrade().unwrap();
        let mut connections = self.connections.lock().await;
        match connections.get_mut(n) {
            Some(p) => p.ping_nanos = Some(ping_nanos),
            None => warn!(node.ll, "unable to set ping: no connection to {}", n),
        };
    }

    pub(super) async fn disconnect(&self, n: &PeerID) {
        let node = Weak::clone(unsafe {&*self.node_ref.get()}).upgrade().unwrap();
        let mut connections = self.connections.lock().await;
        if connections.remove(n).is_none() {
            warn!(node.ll, "Already disconnected: {}", n);
        }

        // TODO [#21]: Send quit packet when disconnecting
        // We will have to add a parameter in this function
    }

    pub(super) async fn insert(&self, n: PeerID, mut s: TcpStream, addr: String) {
        let mut connections = self.connections.lock().await;
        let (mut read_stream, write_stream) = s.into_split();
        let peer = PeerInfo {
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
                    warn!(node.upgrade().unwrap().ll, "packet size too large");
                    unimplemented!("Recovery of packet size too large");
                }
                let mut packet = Vec::with_capacity(packet_size as usize);
                unsafe {packet.set_len(packet_size as usize)};
                read_stream.read_exact(&mut packet).await.unwrap();

                // Parse packet
                let packet: Packet = match Parcel::from_raw_bytes(&packet, &PROTOCOL_SETTINGS) {
                    Ok(p) => p,
                    Err(e) => {
                        warn!(node.upgrade().unwrap().ll, "Failed to parse packet {:?}", e);
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

    pub(super) async fn prepare_discover_peers_response(&self, n: &PeerID, p: DiscoverPeersPacket) -> DiscoverPeersRespPacket {
        let connections = self.connections.lock().await;
        let mut peers = Vec::new();
        for (peer_id, peer) in connections.iter() {
            if peer_id.matches(&p.target, &p.mask) {
                peers.push((peer_id.clone(), peer.addr.clone()));
            }
        }
        std::mem::drop(connections); // Release the lock

        // TODO [#23]: Avoid returning the peer that makes the request when certain conditions are met

        // TODO [#28]: Create an offline_peers store to return better results
        // In order to increase the strenght of the network
        use rand::seq::SliceRandom;
        peers.shuffle(&mut OsRng);

        let max_len = std::cmp::min(p.limit, MAX_PEERS_RETURNED) as usize;
        while peers.len() > max_len {
            peers.remove(max_len * 2/3);
        }

        DiscoverPeersRespPacket {
            request_id: p.request_id,
            peers,
        }
    }

    /// Returns a list of all connected node IDs
    pub(super) async fn peers(&self) -> Vec<PeerID> {
        let connections = self.connections.lock().await;
        connections.keys().cloned().collect()
    }

    pub(super) async fn contains(&self, peer_id: &PeerID) -> bool {
        let connections = self.connections.lock().await;
        connections.contains_key(peer_id)
    }

    pub(super) async fn peers_on_bucket(&self, bucket_level: usize, bucket_id: usize) -> Vec<PeerID> {
        let connections = self.connections.lock().await;
        connections.keys().cloned().filter(|n| n.bucket(&self.our_peer_id).map(|l| l == (bucket_level, bucket_id)).unwrap_or(false)).collect()
    }

    pub(super) async fn peers_on_bucket_and_under(&self, bucket_level: usize) -> Vec<PeerID> {
        let connections = self.connections.lock().await;
        connections.keys().cloned().filter(|n| n.bucket(&self.our_peer_id).map(|l| l.0 <= bucket_level).unwrap_or(false)).collect()
    }

    pub(super) async fn refresh_buckets(&self) {
        'higher: for bucket_level in 0..128 {
            for bucket_id in 0..3 {
                let peers = self.peers_on_bucket(bucket_level, bucket_id).await;

                if peers.len() <= KADEMLIA_BUCKET_SIZE {
                    debug!(self.ll, "Bucket {bucket_level} {} is missing peers", (['A', 'B', 'C'][bucket_id]));

                    let node = Weak::clone(unsafe {&*self.node_ref.get()}).upgrade().unwrap();
                    spawn(async move {
                        node.discover_peers_in_bucket(bucket_level, bucket_id).await;
                    });
                }

                if peers.is_empty() {
                    debug!(self.ll, "Bucket {bucket_level} {} is empty, there is no point in trying to fill lower buckets", (['A', 'B', 'C'][bucket_id]));

                    // TODO [#29]: Try to fill lower buckets when appropriate

                    break 'higher;
                }
            }
        }
    }

    pub(super) async fn debug_buckets(&self) {
        let mut message = String::from("Buckets:\n");
        for bucket_level in 0..128 {
            for bucket_id in 0..3 {
                let peers = self.peers_on_bucket(bucket_level, bucket_id).await;
                if peers.is_empty() {
                    continue;
                }
                message.push_str(&format!("{}-{} ({}): ", bucket_level, ['A', 'B', 'C'][bucket_id], peers.len()));
                for peer in peers.iter() {
                    message.push_str(&format!("{}, ", peer));
                }
                message.push('\n');
            }
        }
        if message.is_empty() {
            message = "No buckets".to_string();
        }
        log::info!("{}", message);
    }
}
