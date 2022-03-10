use std::{sync::Arc, io::Write};
use log::*;
use async_mutex::Mutex;
use async_channel::{Sender, Receiver};

mod commands;
use commands::*;
mod stream;
use stream::*;

static mut RUNNING_COMMAND_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

lazy_static::lazy_static!(
    static ref LISTENERS: Arc<Mutex<Vec<Sender<TcpStream>>>> = Arc::new(Mutex::new(Vec::new()));
);

// TODO: error handling
#[cfg(feature = "test")]
async fn connect(addr: String) -> TcpStream {
    if !addr.starts_with("local-") {
        panic!("Only local-* addresses are supported for testing");
    }
    
    let (our_stream, their_stream) = TcpStream::new();

    LISTENERS.lock().await[addr[7..].parse::<usize>().unwrap()].send(their_stream).await.unwrap();
    
    our_stream
}

#[cfg(not(feature = "test"))]
async fn connect(_addr: String) -> TcpStream {
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

    let mut connections = Vec::new();

    // For now we generate random addresses but in the future we will fetch them
    for _ in 0..5 {
        use rand::Rng;

        let n = rand::thread_rng().gen_range(0..1000);
        let addr = format!("local-{}", n);
        connections.push(connect(addr));
    }

    loop {
        let command = command_receiver.wait_command().await;

        match command {
            Command::ConnCount => {
                info!("{} connections", connections.len());
            }
            command => info!("{:?}", command),
        }

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
    for _ in 0..1000 {
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
