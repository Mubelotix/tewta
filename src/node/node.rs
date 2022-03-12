use super::*;

#[derive(Default)]
pub struct Node {
    connections: ConnectionPool,

    ping_id_counter: Counter,

    on_ping_packet: EventListeners<(PeerID, PingPacket)>,
    on_pong_packet: EventListeners<(PeerID, PingPacket)>,
}

impl Node {
    pub async fn new() -> Arc<Node> {
        let node = Arc::new(Node::default());

        node.connections.set_node_ref(Arc::downgrade(&node));
        node.bootstrap_peers().await;

        node
    }

    async fn bootstrap_peers(&self) {
        for _ in 0..50 {
            use rand::Rng;
    
            let n = rand::thread_rng().gen_range(0..if cfg!(feature = "onlyfive") { 5 } else { 1000 });
            let addr = format!("local-{}", n);
            if let Some(connection) = connect(addr).await {
                // TODO [$622cc9e9961a8f0008186a34]: restore that line
                // self.connections.insert(n, connection).await;
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
        // TODO [$622cc9e9961a8f0008186a35]: Init connection
        // Say hi to the peer, get its ID and push it to the pool
        // self.connections.insert(n, s).await;
    }

    /// Handles a packet by executing the default associated implementation and notifying event listeners.
    /// 
    /// This method will be called concurrently, but only for different nodes.
    /// Meaning packets from the same node will be handled serially.
    // TODO [$622cc9e9961a8f0008186a36]: Could we use a `&PeerID` to spare clones?
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
        }
    }
}
