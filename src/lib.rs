// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

#![allow(clippy::uninit_vec)]
#![allow(clippy::uninit_assumed_init)]

#[macro_use]
pub mod logging;
pub mod commands;
pub mod stream;
pub mod node;
pub mod packets;
pub mod peers;
pub mod util;
pub mod signed_data;
pub mod prelude;
pub mod account;
pub mod segmented_array;
pub mod hash;

use prelude::*;

#[cfg(feature = "test")]
pub static mut NODE_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(feature = "test")]
pub static mut RUNNING_COMMAND_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(feature = "test")]
lazy_static::lazy_static!(
    pub static ref LISTENERS: Arc<Mutex<Vec<Sender<TcpStream>>>> = Arc::new(Mutex::new(Vec::new()));
);

// TODO [#1]: error handling
#[cfg(feature = "test")]
pub async fn connect(addr: String) -> Option<TcpStream> {
    if !addr.starts_with("local-") {
        panic!("Only local-* addresses are supported for testing");
    }
    
    let (our_stream, their_stream) = TcpStream::new();

    let listeners = LISTENERS.lock().await;
    let sender = match listeners.get(addr[6..].parse::<usize>().unwrap()) {
        Some(s) => s,
        None => return None,
    };

    match sender.send(their_stream).await {
        Ok(_) => Some(our_stream),
        Err(e) => panic!("{}", e),
    }
}

#[cfg(not(feature = "test"))]
pub async fn connect(_addr: String) -> Option<TcpStream> {
    unimplemented!()
}

pub mod constants {
    pub const PROTOCOL_VERSION: (u32, u32, u32) = (0, 0, 1);
    pub const MAX_PACKET_SIZE: u32 = 1_000_000;
    pub const MAX_DISCOVERY_PEERS_RETURNED: u16 = 64;
    pub const MAX_DHT_VALUES_RETURNED: u16 = 64;
    pub const MAX_DHT_PEERS_RETURNED: u16 = 32;
    pub const KADEMLIA_BUCKET_SIZE: usize = 8;
    pub const KADEMLIA_ALPHA: usize = 3;
    #[cfg(feature = "test")]
    pub const RSA_KEY_LENGHT: usize = 1024;
    #[cfg(not(feature = "test"))]
    pub const RSA_KEY_LENGHT: usize = 4096;
    pub const PROTOCOL_SETTINGS: protocol::Settings = protocol::Settings {
        byte_order: protocol::ByteOrder::LittleEndian,
    };
}
