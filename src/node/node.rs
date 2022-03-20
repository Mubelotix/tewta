use super::*;

pub struct Node {
    connections: ConnectionPool,
    dht: DhtStore,
    rsa_private_key: RsaPrivateKey,
    rsa_public_key: RsaPublicKey,
    peer_id: PeerID,
    addr: String,

    pub(super) ll: LogLevel,

    // Counters
    ping_id_counter: Counter,
    discover_peer_req_counter: Counter,

    // Event listeners
    on_ping_packet: EventListeners<(PeerID, PingPacket)>,
    on_pong_packet: EventListeners<(PeerID, PingPacket)>,
    on_discover_peers_packet: EventListeners<(PeerID, DiscoverPeersPacket)>,
    on_discover_peers_resp_packet: EventListeners<(PeerID, DiscoverPeersRespPacket)>,
    on_find_dht_value_packet: EventListeners<(PeerID, FindDhtValuePacket)>,
    on_find_dht_value_resp_packet: EventListeners<(PeerID, FindDhtValueRespPacket)>,
    on_find_peer_packet: EventListeners<(PeerID, FindPeerPacket)>,
    on_find_peer_resp_packet: EventListeners<(PeerID, FindPeerRespPacket)>,
    on_store_dht_value_packet: EventListeners<(PeerID, StoreDhtValuePacket)>,
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

            on_ping_packet: EventListeners::default(),
            on_pong_packet: EventListeners::default(),
            on_discover_peers_packet: EventListeners::default(),
            on_discover_peers_resp_packet: EventListeners::default(),
            on_find_dht_value_packet: EventListeners::default(),
            on_find_dht_value_resp_packet: EventListeners::default(),
            on_find_peer_packet: EventListeners::default(),
            on_find_peer_resp_packet: EventListeners::default(),
            on_store_dht_value_packet: EventListeners::default(),
        });

        node.connections.set_node_ref(Arc::downgrade(&node));

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
                                node.connections.disconnect(peer_id).await;
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
        for _ in 0..50 {    
            let n = rand::thread_rng().gen_range(0..if cfg!(feature = "onlyfive") { 5 } else { 50 });
            let addr = format!("local-{}", n);
            if let Some(s) = connect(addr).await {
                self.on_connection(s).await;
            }

            if self.connections.len().await >= 5 {
                break;
            }

            // TODO [#6]: remove dupes
        }
    }

    pub(super) async fn discover_peers_in_bucket(&self, bucket_level: usize, bucket_id: usize) {
        assert!(bucket_level < 128 && bucket_id < 3);

        let target = self.peer_id.generate_in_bucket(bucket_level, bucket_id);
        let mut mask = vec![0xFFu8; bucket_level.div_euclid(4)];
        match bucket_level.rem_euclid(4) {
            0 => mask.push(0b11000000),
            1 => mask.push(0b11110000),
            2 => mask.push(0b11111100),
            3 => mask.push(0b11111111),
            _ => unsafe { unreachable_unchecked() },
        }

        let mut providers = self.connections.peers_on_bucket_and_under(bucket_level).await;
        let mut candidates: Vec<(PeerID, String)> = Vec::new();
        let mut missing_peers = KADEMLIA_BUCKET_SIZE - self.connections.peers_on_bucket(bucket_level, bucket_id).await.len();

        while missing_peers > 0 {
            if let Some((peer_id, addr)) = candidates.pop() {
                if !peer_id.matches(&target, &mask) {
                    warn!(self.ll, "Response contains peers that do not match request");
                }
                // TODO [#30]: close connection properly
                let s = match connect(addr).await {
                    Some(s) => s,
                    None => continue,
                };
                let r = match handshake(s, &self.addr, &self.peer_id, &self.rsa_public_key, &self.rsa_private_key, self.ll.clone()).await {
                    Ok(r) => r,
                    Err(e) => {
                        error!(self.ll, "Handshake failed: {:?}", e);
                        return;
                    }
                };
                if r.their_peer_id != peer_id {
                    warn!(self.ll, "PeerID at this address changed");
                    continue;
                }
                info!(self.ll, "Successfully discovered one peer ({})", r.their_peer_id);
                missing_peers -= 1;
                let _ = self.connections.insert(r.their_peer_id, r.stream, r.their_addr).await;
            } else if let Some(provider) = providers.pop() {
                let request_id = self.discover_peer_req_counter.next();
                let p = Packet::DiscoverPeers(DiscoverPeersPacket {
                    request_id,
                    target: target.clone(),
                    mask: mask.clone(),
                    limit: MAX_DISCOVERY_PEERS_RETURNED,
                });
    
                // TODO [#31]: Add timeout
    
                let resp_receiver = self.on_discover_peers_resp_packet.listen().await;
                self.connections.send_packet(&provider, p).await;
    
                loop {
                    let (n, resp) = resp_receiver.recv().await.unwrap();
                    if resp.request_id == request_id && n == provider {
                        candidates = resp.peers;
                        let connected_peers = self.connections.peers().await;
                        candidates.retain(|(peer_id, _)| !connected_peers.contains(peer_id));
                        break;
                    }
                }
            } else {
                warn!(self.ll, "No providers available");
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
            c => log::info!("{:?}", c),
        }
    }

    pub async fn on_connection(&self, s: TcpStream) {
        // TODO [#17]: Add timeout on handshake

        let r = match handshake(s, &self.addr, &self.peer_id, &self.rsa_public_key, &self.rsa_private_key, self.ll.clone()).await {
            Ok(r) => r,
            Err(HandshakeError::SamePeer) => return,
            Err(e) => {
                // TODO [#18]: We should send quit on errors before terminating the connection
                error!(self.ll, "Handshake failed: {:?}", e);
                return;
            }
        };

        trace!(self.ll, "successful handshake");

        let _ = self.connections.insert(r.their_peer_id, r.stream, r.their_addr).await;
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

            _ => todo!(),
        }
    }
}
