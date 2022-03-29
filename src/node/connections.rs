// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

use crate::prelude::*;

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
    read_stream_task: tokio::task::JoinHandle<()>,

    // TODO [#43]: Hold reputation data here in the PeerInfo struct
}

pub struct ConnectionPool {
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

    /// # Safety
    /// 
    /// This method MUST be called right after the pool creation.
    /// This method must be called exactly once.
    /// No other reference to this pool should be held when calling this method.
    pub unsafe fn set_node_ref(&self, node_ref: Weak<Node>) {
        *self.node_ref.get() = node_ref;
    }

    fn get_node(&self) -> Option<Arc<Node>> {
        // JUSTIFICATION
        //  Benefit
        //      We have to use UnsafeCell in order to have a reference to our parent struct.
        //  Soundness
        //      set_node_ref shall never be called more than once, so node_ref can now be considered immutable.
        unsafe {
            Weak::clone(&*self.node_ref.get()).upgrade()
        }
    }

    /// # Panics
    /// 
    /// Panics if packet is a quit packet. In that case, you should use `ConnectionPool::disconnect` instead.
    pub async fn send_packet(&self, peer_id: &PeerID, p: Packet) {
        assert!(!matches!(p, Packet::Quit(_)));
        self.send_packet_unchecked(peer_id, p).await
    }

    async fn send_packet_unchecked(&self, peer_id: &PeerID, p: Packet) {
        let node = self.get_node().unwrap();

        // Serialize packet
        let p = match p.raw_bytes(&PROTOCOL_SETTINGS) {
            Ok(p) => p,
            Err(e) => {
                error!(node.ll, "{:?}", e);
                return;
            }
        };

        // Lock peer
        let mut connections = self.connections.lock().await;
        let peer = match connections.get_mut(peer_id) {
            Some(s) => s,
            None => {
                warn!(node.ll, "no connection to {}", peer_id);
                return;
            },
        };

        // Write packet prefixed with length
        let len = p.len() as u32;
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&len.to_be_bytes());
        peer.write_stream.write_all(&buf).await.unwrap();
        peer.write_stream.write_all(&p).await.unwrap();
        trace!(node.ll, "packet written to {}: {:?}", peer_id, p);
    }

    pub async fn set_ping(&self, n: &PeerID, ping_nanos: usize) {
        let node = self.get_node().unwrap();
        let mut connections = self.connections.lock().await;
        match connections.get_mut(n) {
            Some(p) => p.ping_nanos = Some(ping_nanos),
            None => warn!(node.ll, "unable to set ping: no connection to {}", n),
        };
    }

    pub async fn disconnect(&self, n: PeerID, quit_packet: QuitPacket) {
        let node = self.get_node().unwrap();

        // Send the quit packet
        self.send_packet_unchecked(&n, Packet::Quit(quit_packet)).await;

        // Remove the peer and stop reading its packets
        let mut connections = self.connections.lock().await;
        match connections.remove(&n) {
            Some(peer) => {
                peer.read_stream_task.abort();
                std::mem::drop(connections);
                node.on_disconnect.event(n).await;
            },
            None => warn!(node.ll, "already disconnected {}", n),
        }
    }

    pub async fn insert(&self, peer_id: PeerID, mut r: ReadHalf, mut w: WriteHalf, addr: String) -> Result<(), ()> {
        let mut connections = self.connections.lock().await;
        if connections.contains_key(&peer_id) {
            let p = Packet::Quit(QuitPacket {
                reason_code: String::from("InsertError::AlreadyConnected"),
                message: None,
                report_fault: false,
            });
            let p = p.raw_bytes(&PROTOCOL_SETTINGS).expect("Failed to serialize packet");
            let plen = p.len() as u32;
            let mut plen_buf = [0u8; 4];
            plen_buf.copy_from_slice(&plen.to_be_bytes());
            let _ = w.write_all(&plen_buf).await;
            let _ = w.write_all(&p).await;

            return Err(());
        }

        // Listen for messages from the remote node
        let node = Weak::clone(unsafe {&*self.node_ref.get()});
        let peer_id2 = peer_id.clone();
        let handle = tokio::spawn(async move {
            loop {
                // TODO [#22]: Aes encryption
                // For receiving and sending

                // Read packet
                let packet_size = r.read_u32().await.unwrap();
                if packet_size >= MAX_PACKET_SIZE {
                    warn!(node.upgrade().unwrap().ll, "packet size too large");
                    unimplemented!("Recovery of packet size too large");
                }
                let mut packet = Vec::with_capacity(packet_size as usize);
                unsafe {packet.set_len(packet_size as usize)};
                r.read_exact(&mut packet).await.unwrap();

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
                node.upgrade().unwrap().on_packet(peer_id2.clone(), packet).await;
            }
        });

        // Insert peer
        let peer = PeerInfo {
            addr,
            write_stream: w,
            ping_nanos: None,
            read_stream_task: handle,
        };
        connections.insert(peer_id.clone(), peer);

        Ok(())
    }

    pub async fn len(&self) -> usize {
        let connections = self.connections.lock().await;
        connections.len()
    }

    pub async fn prepare_discover_peers_response(&self, _n: &PeerID, p: DiscoverPeersPacket) -> DiscoverPeersRespPacket {
        let connections = self.connections.lock().await;
        let mut peers = Vec::new();
        for (peer_id, peer) in connections.iter() {
            if peer_id.matches(&p.target, &p.mask) {
                peers.push((peer_id.clone(), peer.addr.clone()));
            }
        }
        std::mem::drop(connections); // Release the lock

        // TODO [#55]: Use the n parameter in prepare_discover_peers_response

        // TODO [#23]: Avoid returning the peer that makes the request when certain conditions are met

        // TODO [#28]: Create an offline_peers store to return better results
        // In order to increase the strenght of the network
        use rand::seq::SliceRandom;
        peers.shuffle(&mut OsRng);

        let max_len = std::cmp::min(p.limit, MAX_DISCOVERY_PEERS_RETURNED) as usize;
        while peers.len() > max_len {
            peers.remove(max_len * 2/3);
        }

        DiscoverPeersRespPacket {
            request_id: p.request_id,
            peers,
        }
    }

    /// Returns a list of all connected node IDs
    pub async fn peers(&self) -> Vec<PeerID> {
        let connections = self.connections.lock().await;
        connections.keys().cloned().collect()
    }

    pub async fn peers_with_addrs(&self) -> Vec<(PeerID, String)> {
        let connections = self.connections.lock().await;
        connections.iter().map(|(n, p)| (n.clone(), p.addr.clone())).collect()
    }

    pub async fn contains(&self, peer_id: &PeerID) -> bool {
        let connections = self.connections.lock().await;
        connections.contains_key(peer_id)
    }

    pub async fn peers_on_bucket(&self, bucket_level: usize, bucket_id: usize) -> Vec<PeerID> {
        let connections = self.connections.lock().await;
        connections.keys().cloned().filter(|n| n.bucket(&self.our_peer_id).map(|l| l == (bucket_level, bucket_id)).unwrap_or(false)).collect()
    }

    pub async fn peers_on_bucket_and_under(&self, bucket_level: usize) -> Vec<PeerID> {
        let connections = self.connections.lock().await;
        connections.keys().cloned().filter(|n| n.bucket(&self.our_peer_id).map(|l| l.0 <= bucket_level).unwrap_or(false)).collect()
    }

    /// Refresh buckets and discovers new peers.  
    /// This will return immediately as tasks are spawned.
    pub async fn refresh_buckets(&self) {
        'higher: for bucket_level in 0..128 {
            for bucket_id in 0..3 {
                let peers = self.peers_on_bucket(bucket_level, bucket_id).await;

                if peers.len() < KADEMLIA_BUCKET_SIZE {
                    trace!(self.ll, "Bucket {bucket_level} {} is missing peers ({}/{})", (['A', 'B', 'C'][bucket_id]), (peers.len()), KADEMLIA_BUCKET_SIZE);

                    let node = self.get_node().unwrap();
                    spawn(async move {
                        node.discover_peers_in_bucket(bucket_level, bucket_id).await;
                    });
                }

                if peers.is_empty() {
                    trace!(self.ll, "Bucket {bucket_level} {} is empty, there is no point in trying to fill lower buckets", (['A', 'B', 'C'][bucket_id]));

                    // TODO [#29]: Try to fill lower buckets when appropriate

                    break 'higher;
                }
            }
        }
    }

    pub async fn debug_buckets(&self) {
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
