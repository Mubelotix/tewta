use super::*;

pub struct Node {
    connections: ConnectionPool,

    on_ping_packet: EventListeners<PingPacket>,
    on_pong_packet: EventListeners<PingPacket>,
}

impl Node {
    pub async fn new() -> Arc<Node> {
        let node = Arc::new(Node {
            connections: ConnectionPool::new(),

            on_ping_packet: EventListeners::new(),
            on_pong_packet: EventListeners::new(),
        });

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
                self.connections.insert(n, connection).await;
            }

            if self.connections.len().await >= 5 {
                break;
            }

            // TODO: remove dupes
        }
    }

    pub async fn on_command(&self, c: Command) {
        match c {
            Command::ConnCount => {
                info!("{} connections ({:?})", self.connections.len().await, self.connections.connected_nodes().await);
            }
            Command::Ping { node_id } => {
                let p = PingPacket {ping_id: 666};
                self.connections.send_packet(node_id, Packet::Ping(p)).await;
            }
            c => info!("{:?}", c),
        }
    }

    pub async fn on_connection(&self, n: NodeID, s: TcpStream) {
        self.connections.insert(n, s).await;
    }

    /// Handles a packet by executing the default associated implementation and notifying event listeners.
    /// 
    /// This method will be called concurrently, but only for different nodes.
    /// Meaning packets from the same node will be handled serially.
    pub async fn on_packet(&self, n: NodeID, p: Packet) {
        match p {
            Packet::Ping(p) => {
                let response = Packet::Pong(p);
                self.connections.send_packet(n, response).await;

                self.on_ping_packet.event(p).await;
            }
            Packet::Pong(p) => {
                self.on_pong_packet.event(p).await;
            }
        }
    }
}
