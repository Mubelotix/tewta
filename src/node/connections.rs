use super::*;

#[cfg(not(feature = "test"))]
type ReadHalf<'a> = tokio::net::tcp::ReadHalf<'a>;
#[cfg(not(feature = "test"))]
type WriteHalf<'a> = tokio::net::tcp::WriteHalf<'a>;
#[cfg(feature = "test")]
type ReadHalf<'a> = crate::stream::testing::TestReadHalf;
#[cfg(feature = "test")]
type WriteHalf<'a> = crate::stream::testing::TestWriteHalf;

pub(super) struct ConnectionPool<'a> {
    connections: Mutex<HashMap<NodeID, WriteHalf<'a>>>,
    node_ref: UnsafeCell<Weak<Node<'a>>>,
}

unsafe impl<'a> Sync for ConnectionPool<'a> {}

impl<'a> ConnectionPool<'a> {
    pub(super) fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
            node_ref: UnsafeCell::new(Weak::new()),
        }
    }

    pub(super) fn set_node_ref(&self, node_ref: Weak<Node<'a>>) {
        unsafe {
            *self.node_ref.get() = node_ref;
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

    // TODO: n should be removed
    pub(super) async fn insert(&self, n: NodeID, mut s: TcpStream) {
        let mut connections = self.connections.lock().await;
        let (mut read_stream, write_stream) = s.split();
        connections.insert(n, write_stream);

        // Listen for messages from the remote node
        tokio::spawn(async move {
            use rand::Rng;
            //let random = rand::thread_rng().gen_range(0..1000);
            let mut x = 0;
            loop {
                // wait readable 
                let future = read_stream.readable();
                let r = future.await;

                info!("SOMETHING IS READABLE");

                /*// Read packet
                let packet_size = read_stream.read_u32().await.unwrap();
                // TODO: Add setting for max packet size
                if packet_size >= 1_000_000 {
                    error!("packet size too large");
                    unimplemented!("Recovery of packet size too large");
                }
                let mut packet = Vec::with_capacity(packet_size as usize);
                read_stream.read_exact(&mut packet).await.unwrap();

                println!("Packet obtained");

                // Parse packet
                let packet: Packet = match Parcel::from_raw_bytes(&packet, &ProtocolSettings::default()) {
                    Ok(p) => p,
                    Err(e) => {
                        error!("{:?}", e);
                        continue;
                    },
                }; */
            }
        });
    }

    pub(super) async fn len(&self) -> usize {
        let connections = self.connections.lock().await;
        connections.len()
    }

    /// Returns a list of all connected node IDs
    pub(super) async fn connected_nodes(&self) -> Vec<NodeID> {
        let connections = self.connections.lock().await;
        connections.keys().cloned().collect()
    }
}
