use super::*;

pub struct Node {
    connections: ConnectionPool,
    rsa_private_key: RsaPrivateKey,
    rsa_public_key: RsaPublicKey,
    peer_id: PeerID,
    addr: String,

    pub log_level: LogLevel,

    ping_id_counter: Counter,
    peer_req_id_counter: Counter,

    on_ping_packet: EventListeners<(PeerID, PingPacket)>,
    on_pong_packet: EventListeners<(PeerID, PingPacket)>,
    on_find_peers_packet: EventListeners<(PeerID, FindPeersPacket)>,
    on_return_peers_packet: EventListeners<(PeerID, ReturnPeersPacket)>,
}

impl Node {
    pub async fn new(addr: String) -> Arc<Node> {
        //debug!("Generating RSA key pair...");
        let private_key = RsaPrivateKey::new(&mut OsRng, RSA_KEY_LENGHT).expect("failed to generate a key");
        let public_key = RsaPublicKey::from(&private_key);
        //debug!("RSA keys generated!");

        let node = Arc::new(Node {
            connections: ConnectionPool::default(),
            peer_id: PeerID::from(&public_key),
            addr,
            rsa_private_key: private_key,
            rsa_public_key: public_key,

            log_level: LogLevel::from(0),

            ping_id_counter: Counter::default(),
            peer_req_id_counter: Counter::default(),

            on_ping_packet: EventListeners::default(),
            on_pong_packet: EventListeners::default(),
            on_find_peers_packet: EventListeners::default(),
            on_return_peers_packet: EventListeners::default(),
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

                let peer_ids = node.connections.connected_nodes().await;
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
                                warn!(node.log_level, "Connection timed out, disconnecting");
                                node.connections.disconnect(peer_id).await;
                            },
                        }
                    });
                }
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

    pub async fn on_command(&self, c: Command) {
        match c {
            Command::Conns => {
                log::info!("{} connections ({:?})", self.connections.len().await, self.connections.connected_nodes().await);
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
                self.log_level.set(level);
            }
            c => log::info!("{:?}", c),
        }
    }

    pub async fn on_connection(&self, s: TcpStream) {
        // TODO [#17]: Add timeout on handshake

        let r = match handshake(s, &self.addr, &self.peer_id, &self.rsa_public_key, &self.rsa_private_key, self.log_level.clone()).await {
            Ok(r) => r,
            Err(e) => {
                // TODO [#18]: We should send quit on errors before terminating the connection
                error!(self.log_level, "Handshake failed: {:?}", e);
                return;
            }
        };

        info!(self.log_level, "successful handshake");

        // TODO [#24]: Set addr from handshake
        self.connections.insert(r.their_peer_id, r.stream, r.their_addr).await;
    }

    /// Handles a packet by executing the default associated implementation and notifying event listeners.
    /// 
    /// This method will be called concurrently, but only for different nodes.
    /// Meaning packets from the same node will be handled serially.
    // TODO [#12]: Could we use a `&PeerID` to spare clones?
    pub async fn on_packet(&self, n: PeerID, p: Packet) {
        debug!(self.log_level, "Received packet {:?}", p);

        match p {
            Packet::Ping(p) => {
                let response = Packet::Pong(p);
                self.connections.send_packet(&n, response).await;

                self.on_ping_packet.event((n, p)).await;
            }
            Packet::Pong(p) => {
                self.on_pong_packet.event((n, p)).await;
            }
            Packet::FindPeers(p) => {
                let response = self.connections.prepare_find_peers_response(&n, p.clone()).await;
                self.connections.send_packet(&n, Packet::ReturnPeers(response)).await;
                
                self.on_find_peers_packet.event((n, p)).await;
            },
            Packet::ReturnPeers(p) => {
                if p.peers.len() >= MAX_PEERS_RETURNED as usize {
                    warn!(self.log_level, "Too many peers returned, dropping");
                    return;
                }

                // TODO [#25]: Sort the peers by distance when received
                // So that we don't duplicate the work by sending the packet to multiple event handlers

                self.on_return_peers_packet.event((n, p)).await;
            }
            _ => todo!(),
        }
    }
}
