#![allow(clippy::uninit_vec)]
#![allow(clippy::uninit_assumed_init)]

use std::{sync::Arc, io::Write};
use log::*;
use async_mutex::Mutex;
use async_channel::{Sender, Receiver};

pub mod commands;
use commands::*;
pub mod stream;
use stream::*;
pub mod node;
use node::*;
pub mod packets;
use packets::*;
pub mod peers;
use peers::*;
pub mod util;
#[macro_use]
pub mod logging;

static mut RUNNING_COMMAND_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

lazy_static::lazy_static!(
    static ref LISTENERS: Arc<Mutex<Vec<Sender<TcpStream>>>> = Arc::new(Mutex::new(Vec::new()));
);

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

// TODO [#1]: error handling
#[cfg(feature = "test")]
async fn connect(addr: String) -> Option<TcpStream> {
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
async fn connect(_addr: String) -> Option<TcpStream> {
    unimplemented!()
}

pub async fn run(addr: String, connection_receiver: Receiver<TcpStream>, command_receiver: CommandReceiver) {
    let node = Node::new(addr).await;

    let node2 = Arc::clone(&node);
    #[cfg(feature = "test")]
    tokio::spawn(async move {
        loop {
            let stream = connection_receiver.recv().await.unwrap();
            node2.on_connection(stream).await;
        }
    });

    loop {
        let command = command_receiver.wait_command().await;
        node.on_command(command).await;

        // Print command input chars if no command is running anymore
        if unsafe { RUNNING_COMMAND_COUNTER.fetch_sub(1, std::sync::atomic::Ordering::Relaxed) } == 1 {
            print!("\x1b[32m>>> \x1b[0m");
            std::io::stdout().flush().unwrap();
        }
    }

}

#[tokio::main]
async fn main() {
    thousand_nodes().await;
}

async fn thousand_nodes() {
    env_logger::init();

    let mut command_senders = Vec::new();
    for i in 0..if cfg!(feature="onlyfive") {5} else {50} {
        let (command_receiver, command_sender) = CommandReceiver::new();
        command_senders.push(command_sender);
        let (connection_sender, connection_receiver) = async_channel::unbounded();
        LISTENERS.lock().await.push(connection_sender);
        tokio::spawn(run(format!("local-{}", i), connection_receiver, command_receiver));
    }

    log::info!("All nodes running");

    print!("\x1b[32m>>> \x1b[0m");
    loop {
        let mut raw_command = String::new();
        std::io::stdout().flush().unwrap();
        std::io::stdin().read_line(&mut raw_command).unwrap();
        match Command::parse(&raw_command) {
            Ok((destinators, command)) => {
                for destinator in destinators {
                    command_senders[destinator].send(command.clone()).await.unwrap();
                    unsafe { RUNNING_COMMAND_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
                }
            }
            Err(e) => {
                eprintln!("{}", e);
                match e {
                    CommandParsingError::Clap(e) if e.kind == structopt::clap::ErrorKind::HelpDisplayed => {
                        print!("\x1b[32m>>> \x1b[0m");
                    }
                    _ => print!("\x1b[31m>>> \x1b[0m"), 
                };
            },
        }
    }
}
