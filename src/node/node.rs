// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

use crate::prelude::*;

pub struct Node {
    pub connections: ConnectionPool,
    pub dht: DhtStore,
    pub rsa_private_key: RsaPrivateKey,
    pub rsa_public_key: RsaPublicKey,
    pub peer_id: PeerID,
    pub addr: String,

    pub ll: LogLevel,

    // Counters
    pub ping_id_counter: Counter,
    pub discover_peer_req_counter: Counter,
    pub dht_req_counter: Counter,

    // Event listeners
    pub on_ping_packet: EventListeners<(PeerID, PingPacket)>,
    pub on_pong_packet: EventListeners<(PeerID, PingPacket)>,
    pub on_quit_packet: EventListeners<(PeerID, QuitPacket)>,
    pub on_discover_peers_packet: EventListeners<(PeerID, DiscoverPeersPacket)>,
    pub on_discover_peers_resp_packet: EventListeners<(PeerID, DiscoverPeersRespPacket)>,
    pub on_find_dht_value_packet: EventListeners<(PeerID, FindDhtValuePacket)>,
    pub on_find_dht_value_resp_packet: EventListeners<(PeerID, FindDhtValueRespPacket)>,
    pub on_find_peer_packet: EventListeners<(PeerID, FindPeerPacket)>,
    pub on_find_peer_resp_packet: EventListeners<(PeerID, FindPeerRespPacket)>,
    pub on_store_dht_value_packet: EventListeners<(PeerID, StoreDhtValuePacket)>,
}

impl Node {
    pub async fn new(addr: String) -> Arc<Node> {
        //debug!("Generating RSA key pair...");
        let private_key = RsaPrivateKey::new(&mut OsRng, RSA_KEY_LENGHT).expect("failed to generate a key");
        let public_key = RsaPublicKey::from(&private_key);
        let peer_id = PeerID::from(&public_key);
        //debug!("RSA keys generated!");

        let log_level = LogLevel::from(1);

        let node = Arc::new(Node {
            connections: ConnectionPool::new(peer_id.clone(), log_level.clone()),
            dht: DhtStore::default(),
            peer_id,
            addr,
            rsa_private_key: private_key,
            rsa_public_key: public_key,

            ll: log_level,

            ping_id_counter: Counter::default(),
            discover_peer_req_counter: Counter::default(),
            dht_req_counter: Counter::default(),

            on_ping_packet: EventListeners::default(),
            on_pong_packet: EventListeners::default(),
            on_quit_packet: EventListeners::default(),
            on_discover_peers_packet: EventListeners::default(),
            on_discover_peers_resp_packet: EventListeners::default(),
            on_find_dht_value_packet: EventListeners::default(),
            on_find_dht_value_resp_packet: EventListeners::default(),
            on_find_peer_packet: EventListeners::default(),
            on_find_peer_resp_packet: EventListeners::default(),
            on_store_dht_value_packet: EventListeners::default(),
        });

        // JUSTIFICATION
        //  Benefit
        //      We have to use this method in order to give the pool a reference to ourselves.
        //  Soundness
        //      We follow all the method's safety requirements:
        //      - This is called only once,
        //      - This is called right after we create the node,
        //      - This is called as there are no other references to the node
        unsafe {
            node.connections.set_node_ref(Arc::downgrade(&node));
        }

        let node2 = Arc::clone(&node);
        spawn(async move {
            node2.bootstrap_peers().await;
        });

        // Continuously ping peers
        let node2 = Arc::downgrade(&node);
        spawn(async move {
            let node = node2;
            loop {
                sleep(Duration::from_secs(100)).await;

                let node = match node.upgrade() {
                    Some(node) => node,
                    None => break,
                };

                let peer_ids = node.connections.peers().await;
                for peer_id in peer_ids {
                    let node = Arc::clone(&node);
                    spawn(async move {
                        // Send ping
                        let peer_id = &peer_id;
                        let ping_id = node.ping_id_counter.next();
                        let start = Instant::now();
                        node.connections.send_packet(peer_id, Packet::Ping(PingPacket { ping_id })).await;

                        // Receive pong
                        let pong_receiver = node.on_pong_packet.listen().await;
                        let result = timeout(Duration::from_secs(30), async move {
                            loop {
                                let (n, pong) = pong_receiver.recv().await.unwrap();
                                if pong.ping_id == ping_id && &n == peer_id {
                                    break Instant::now().duration_since(start);
                                }
                            }
                        }).await;

                        // Handle result
                        match result {
                            Ok(d) => node.connections.set_ping(peer_id, d.as_nanos() as usize).await,
                            Err(_) => {                                     
                                warn!(node.ll, "Connection timed out, disconnecting {}", peer_id);
                                let quit_packet = QuitPacket {
                                    reason_code: String::from("Timeout"),
                                    message: None,
                                    report_fault: false,
                                };
                                node.connections.disconnect(peer_id, quit_packet).await;
                            },
                        }
                    });
                }
            }
        });

        // Update buckets
        let node2 = Arc::downgrade(&node);
        spawn(async move {
            let node = node2;
            loop {
                sleep(Duration::from_secs(100)).await;

                let node = match node.upgrade() {
                    Some(node) => node,
                    None => break,
                };

                node.connections.refresh_buckets().await;
            }
        });

        node
    }

    async fn bootstrap_peers(&self) {
        let node_count = unsafe {crate::NODE_COUNT.load(std::sync::atomic::Ordering::Relaxed)};

        for _ in 0..node_count {    
            let n = rand::thread_rng().gen_range(0..node_count);
            let addr = format!("local-{}", n);
            if let Some(s) = connect(addr).await {
                self.on_connection(s).await;
            }

            if self.connections.len().await >= 5 {
                break;
            }
        }
    }

    pub async fn on_command(&self, c: Command) {
        match c {
            Command::Conns => {
                log::info!("{} connections", self.connections.len().await);
            }
            Command::Buckets => {
                self.connections.debug_buckets().await;
            }
            Command::RefreshBuckets => {
                self.connections.refresh_buckets().await;
                log::info!("Buckets refreshed");
            }
            Command::Ping { node_id } => {
                // Send ping
                let ping_id = self.ping_id_counter.next();
                let start = Instant::now();
                self.connections.send_packet(&node_id, Packet::Ping(PingPacket { ping_id })).await;

                // Receive pong
                let pong_receiver = self.on_pong_packet.listen().await;
                let result = timeout(Duration::from_secs(15), async move {
                    loop {
                        let (n, pong) = pong_receiver.recv().await.unwrap();
                        if pong.ping_id == ping_id && n == node_id {
                            break Instant::now().duration_since(start);
                        }
                    }
                }).await;

                // Display result
                match result {
                    Ok(d) => log::info!("Ping is {} ms", d.as_millis()),
                    Err(_) => log::info!("Timed out"),
                }
            }
            Command::SetLogLevel { level } => {
                self.ll.set(level);
            }
            Command::Id => {
                log::info!("{}", self.peer_id);
            }
            Command::Store { key, value } => {
                //self.dht.set(key, DhtValue {data: value}).await;
            }
            Command::Find { key } => {
                self.dht_lookup(key).await;
            }
            c => log::info!("{:?}", c),
        }
    }

    pub async fn on_connection(&self, s: TcpStream) {
        trace!(self.ll, "New connection");
        
        let (r, w) = s.into_split();
        let peer_id = match timeout(Duration::from_secs(40), self.handshake(r, w, None)).await {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => {
                warn!(self.ll, "Handshake failed: {:?}", e);
                return;
            }
            Err(_) => {
                warn!(self.ll, "Handshake timed out");
                return;
            }
        };

        trace!(self.ll, "successful handshake with {}", peer_id);
    }

    /// Handles a packet by executing the default associated implementation and notifying event listeners.
    /// 
    /// This method will be called concurrently, but only for different nodes.
    /// Meaning packets from the same node will be handled serially.
    pub async fn on_packet(&self, n: PeerID, p: Packet) {
        trace!(self.ll, "Received packet {:?}", p);

        match p {
            // Peer discovery
            Packet::DiscoverPeers(p) => {
                if p.mask.len() > 32 {
                    warn!(self.ll, "Mask too long, dropping");
                    return;
                }

                let response = self.connections.prepare_discover_peers_response(&n, p.clone()).await;
                self.connections.send_packet(&n, Packet::DiscoverPeersResp(response)).await;
                
                self.on_discover_peers_packet.event((n, p)).await;
            },
            Packet::DiscoverPeersResp(p) => {
                if p.peers.len() > MAX_DISCOVERY_PEERS_RETURNED as usize {
                    warn!(self.ll, "Too many peers returned, dropping");
                    return;
                }

                // TODO [#25]: Sort the peers by distance when received
                // So that we don't duplicate the work by sending the packet to multiple event handlers

                self.on_discover_peers_resp_packet.event((n, p)).await;
            }
            
            // Kademlia DHT
            Packet::FindDhtValue(p) => {
                log::debug!("FindDhtValue in {}", self.peer_id);

                let result = match self.dht.get(&p.key).await {
                    Some(mut values) => {
                        // TODO [#35]: Order results

                        let max_values = min(MAX_DHT_VALUES_RETURNED, p.limit_values);
                        values.truncate(max_values as usize);
                        DhtLookupResult::Found(values)
                    }
                    None => {
                        // TODO [#36]: We might want to use offline nodes too

                        let mut peers = self.connections.peers_with_addrs().await;
                        peers.sort_by_key(|(peer_id, _)| peer_id.distance(&p.key));
                        let max_peers = min(MAX_DHT_PEERS_RETURNED, p.limit_peers);
                        peers.truncate(max_peers as usize);
                        DhtLookupResult::NotFound(peers)
                    }
                };

                self.connections.send_packet(&n, Packet::FindDhtValueResp(FindDhtValueRespPacket {
                    request_id: p.request_id,
                    result
                })).await;

                self.on_find_dht_value_packet.event((n, p)).await;
            }
            Packet::FindDhtValueResp(p) => {
                match &p.result {
                    DhtLookupResult::Found(values) => if values.len() > MAX_DHT_VALUES_RETURNED as usize {
                        warn!(self.ll, "Too many values returned, dropping");
                        return;
                    }
                    DhtLookupResult::NotFound(peers) => if peers.len() > MAX_DHT_PEERS_RETURNED as usize {
                        warn!(self.ll, "Too many peers returned, dropping");
                        return;
                    }
                }

                self.on_find_dht_value_resp_packet.event((n, p)).await;
            }
            Packet::FindPeer(p) => {
                let mut peers = self.connections.peers_with_addrs().await;
                peers.sort_by_key(|(peer_id, _)| peer_id.distance(&p.peer_id));
                let max_peers = min(MAX_DHT_PEERS_RETURNED, p.limit);
                peers.truncate(max_peers as usize);

                self.connections.send_packet(&n, Packet::FindPeerResp(FindPeerRespPacket {
                    request_id: p.request_id,
                    peers
                })).await;

                self.on_find_peer_packet.event((n, p)).await;
            }
            Packet::FindPeerResp(p) => {
                if p.peers.len() > MAX_DHT_PEERS_RETURNED as usize {
                    warn!(self.ll, "Too many peers returned, dropping");
                    return;
                }

                self.on_find_peer_resp_packet.event((n, p)).await;
            }
            Packet::StoreDhtValue(p) => {
                // TODO [#37]: store value

                self.on_store_dht_value_packet.event((n, p)).await;
            }

            // Utility packets
            Packet::Ping(p) => {
                let response = Packet::Pong(p);
                self.connections.send_packet(&n, response).await;

                self.on_ping_packet.event((n, p)).await;
            }
            Packet::Pong(p) => {
                self.on_pong_packet.event((n, p)).await;
            }
            Packet::Quit(p) => {
                if p.report_fault {
                    error!(self.ll, "Peer {} quitted because of us: {}, {:?}", n, p.reason_code, p.message);
                }

                // We shouldn't need to respond with another quit packet but anyway, requiring it in the disconnect method guarantees we never quit without sending a quit packet.
                let quit_packet = QuitPacket {
                    reason_code: String::from("QuitReceived"),
                    message: None,
                    report_fault: false
                };
                self.connections.disconnect(&n, quit_packet).await;

                // TODO [#45]: Refresh buckets to fill an eventual hole after the disconnect
                // This below does not work as the compiler has troubles with some checks
                // self.connections.refresh_buckets().await;

                self.on_quit_packet.event((n, p)).await;
            }

            _ => todo!(),
        }
    }
}
