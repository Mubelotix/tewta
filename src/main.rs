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

static mut RUNNING_COMMAND_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

lazy_static::lazy_static!(
    static ref LISTENERS: Arc<Mutex<Vec<Sender<TcpStream>>>> = Arc::new(Mutex::new(Vec::new()));
);

// TODO: error handling
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

pub async fn run(connection_receiver: Receiver<TcpStream>, command_receiver: CommandReceiver) {
    /*
    use rsa::{PublicKey, RsaPrivateKey, RsaPublicKey, PaddingScheme};
    use rand::rngs::OsRng;

    let mut rng = OsRng; // TODO: Check security
    let bits = 4096;
    let private_key = RsaPrivateKey::new(&mut rng, bits).expect("failed to generate a key");
    let public_key = RsaPublicKey::from(&private_key);
    */

    let node = Node::new().await;

    let node2 = Arc::clone(&node);
    #[cfg(feature = "test")]
    tokio::spawn(async move {
        loop {
            let stream = connection_receiver.recv().await.unwrap();
            use rand::Rng;
            let random = rand::thread_rng().gen_range(1000..100000);
            node2.on_connection(random, stream).await;
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
    for _ in 0..if cfg!(feature="onlyfive") {5} else {1000} {
        let (command_receiver, command_sender) = CommandReceiver::new();
        command_senders.push(command_sender);
        let (connection_sender, connection_receiver) = async_channel::unbounded();
        LISTENERS.lock().await.push(connection_sender);
        tokio::spawn(run(connection_receiver, command_receiver));
    }

    println!("All nodes running");

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
