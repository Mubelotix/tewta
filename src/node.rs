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

struct EventListeners<T: Clone> {
    listeners: Vec<Sender<T>>,
}

impl<T: Clone> EventListeners<T> {
    fn new() -> Self {
        Self {
            listeners: Vec::new(),
        }
    }

    async fn event(&mut self, event: T) {
        for i in (0..self.listeners.len()).rev() {
            // TODO: check if we could optimize by avoiding cloning the last event
            if self.listeners[i].send(event.clone()).await.is_err() {
                self.listeners.remove(i);
            }
        }
    }

    async fn create_listener(&mut self) -> Receiver<T> {
        let (sender, receiver) = async_channel::unbounded();
        self.listeners.push(sender);
        receiver
    }
}

pub struct Node {
    connections: Vec<TcpStream>,
    self_ref: Weak<Mutex<Node>>,

    on_ping_packet: EventListeners<PingPacket>,
    on_pong_packet: EventListeners<PingPacket>,
}

impl Node {
    pub async fn new() -> Arc<Mutex<Node>> {
        let node = Arc::new(Mutex::new(Node {
            connections: Vec::new(),
            self_ref: Weak::new(),

            on_ping_packet: EventListeners::new(),
            on_pong_packet: EventListeners::new(),
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
                self.on_ping_packet.event(p).await;
            }
            Packet::Pong(p) => {
                self.on_pong_packet.event(p).await;
            }
        }
    }
}
