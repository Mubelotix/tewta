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

        node.bootstrap_peers().await;

        node
    }

    async fn bootstrap_peers(&self) {
        for _ in 0..50 {
            use rand::Rng;
    
            let n = rand::thread_rng().gen_range(0..1000);
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
                info!("{} connections", self.connections.len().await);
            }
            c => info!("{:?}", c),
        }
    }

    pub async fn on_connection(&self, s: TcpStream) {

    }

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
