use super::*;

pub struct Node {
    connections: ConnectionPool,

    rsa_private_key: RsaPrivateKey,
    rsa_public_key: RsaPublicKey,
    peer_id: PeerID,

    ping_id_counter: Counter,

    on_ping_packet: EventListeners<(PeerID, PingPacket)>,
    on_pong_packet: EventListeners<(PeerID, PingPacket)>,
}

impl Node {
    pub async fn new() -> Arc<Node> {
        debug!("Generating RSA key pair...");
        let private_key = RsaPrivateKey::new(&mut OsRng, RSA_KEY_LENGHT).expect("failed to generate a key");
        let public_key = RsaPublicKey::from(&private_key);
        debug!("RSA keys generated!");

        let node = Arc::new(Node {
            connections: ConnectionPool::default(),

            peer_id: PeerID::from(&public_key),
            rsa_private_key: private_key,
            rsa_public_key: public_key,

            ping_id_counter: Counter::default(),

            on_ping_packet: EventListeners::default(),
            on_pong_packet: EventListeners::default(),
        });

        node.connections.set_node_ref(Arc::downgrade(&node));

        let node2 = Arc::clone(&node);
        tokio::spawn(async move {
            node2.bootstrap_peers().await;
        });

        node
    }

    async fn bootstrap_peers(&self) {
        for _ in 0..50 {
            use rand::Rng;
    
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
            Command::ConnCount => {
                info!("{} connections ({:?})", self.connections.len().await, self.connections.connected_nodes().await);
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
                    Ok(d) => info!("Ping is {} ms", d.as_millis()),
                    Err(_) => info!("Timed out"),
                }
            }
            c => info!("{:?}", c),
        }
    }

    pub async fn on_connection(&self, s: TcpStream) {
        // TODO: Add timeout on handshake

        let r = match handshake(s, &self.peer_id, &self.rsa_public_key, &self.rsa_private_key).await {
            Ok(r) => r,
            Err(e) => {
                // TODO: We should send quit on errors before terminating the connection
                error!("Handshake failed: {:?}", e);
                return;
            }
        };

        info!("successful handshake");

        self.connections.insert(r.their_peer_id, r.stream).await;
    }

    /// Handles a packet by executing the default associated implementation and notifying event listeners.
    /// 
    /// This method will be called concurrently, but only for different nodes.
    /// Meaning packets from the same node will be handled serially.
    // TODO [#12]: Could we use a `&PeerID` to spare clones?
    pub async fn on_packet(&self, n: PeerID, p: Packet) {
        debug!("Received packet {:?}", p);

        match p {
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
