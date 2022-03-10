use {
    crate::{
        commands::Command,
        stream::TcpStream,
        packets::*,
        connect,
    },
    std::sync::{Arc, Weak},
    async_mutex::Mutex,
    async_channel::{Sender, Receiver},
    log::*,
};

pub struct Node {
    connections: Vec<TcpStream>,
    self_ref: Weak<Mutex<Node>>,

    on_ping_packet: Vec<Sender<PingPacket>>,
    on_pong_packet: Vec<Sender<PingPacket>>,
}

impl Node {
    pub async fn new() -> Arc<Mutex<Node>> {
        let node = Arc::new(Mutex::new(Node {
            connections: Vec::new(),
            self_ref: Weak::new(),

            on_ping_packet: Vec::new(),
            on_pong_packet: Vec::new(),
        }));
        let self_ref = Arc::downgrade(&node);

        {
            let mut node = node.lock().await;
            node.self_ref = self_ref;
            node.bootstrap_peers().await;
        }

        node
    }

    async fn bootstrap_peers(&mut self) {
        for _ in 0..50 {
            use rand::Rng;
    
            let n = rand::thread_rng().gen_range(0..1000);
            let addr = format!("local-{}", n);
            if let Some(connection) = connect(addr).await {
                self.connections.push(connection);
            }

            if self.connections.len() >= 5 {
                break;
            }

            // TODO: remove dupes
        }
    }

    pub async fn on_command(&mut self, c: Command) {
        match c {
            Command::ConnCount => {
                info!("{} connections", self.connections.len());
            }
            c => info!("{:?}", c),
        }
    }

    pub async fn on_connection(&mut self, s: TcpStream) {

    }

    pub async fn on_packet(&mut self, p: Packet) {
        match p {
            Packet::Ping(p) => {
                for sender in self.on_ping_packet.iter() {
                    sender.send(p).await.unwrap();
                }
            }
            Packet::Pong(p) => {
                for sender in self.on_pong_packet.iter() {
                    sender.send(p).await.unwrap();
                }
            }
        }
    }
}
