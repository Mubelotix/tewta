use super::*;

pub(super) struct ConnectionPool {
    connections: Mutex<HashMap<NodeID, TcpStream>>,
}

impl ConnectionPool {
    pub(super) fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
        }
    }

    pub(super) async fn send_packet(&self, n: NodeID, p: Packet) {
        let p = match p.raw_bytes(&protocol::Settings::default()) {
            Ok(p) => p,
            Err(e) => {
                error!("{:?}", e);
                return;
            }
        };

        let mut connections = self.connections.lock().await;

        let tcp_stream = match connections.get_mut(&n) {
            Some(s) => s,
            None => {
                warn!("no connection to {}", n);
                return;
            },
        };

        tcp_stream.write_all(&p).await.unwrap();
    }

    pub(super) async fn insert(&self, n: NodeID, s: TcpStream) {
        let mut connections = self.connections.lock().await;
        connections.insert(n, s);
    }

    pub(super) async fn len(&self) -> usize {
        let connections = self.connections.lock().await;
        connections.len()
    }
}
