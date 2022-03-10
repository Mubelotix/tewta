use {
    crate::{
        commands::Command,
        stream::TcpStream,
        connect,
    },
    std::sync::{Arc, Weak},
    async_mutex::Mutex,
    log::*,
};

// TODO remove this
type Packet = ();

pub struct Node {
    connections: Vec<TcpStream>,
    self_ref: Weak<Mutex<Node>>,
}

impl Node {
    pub async fn new() -> Arc<Mutex<Node>> {
        let node = Arc::new(Mutex::new(Node {
            connections: Vec::new(),
            #[allow(clippy::uninit_assumed_init)]
            self_ref: Weak::new(),
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

    }
}
