use std::{sync::{Arc, Mutex, mpsc}, io::Write};
use log::*;

mod commands;
use commands::*;

static mut RUNNING_COMMAND_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub trait Connection {

}

pub struct VirtualTcp {
    incoming: Arc<Mutex<Vec<u8>>>,
    outgoing: Arc<Mutex<Vec<u8>>>,
}

impl VirtualTcp {
    pub fn new_channel() -> (VirtualTcp, VirtualTcp) {
        let incoming = Arc::new(Mutex::new(Vec::new()));
        let outgoing = Arc::new(Mutex::new(Vec::new()));
        (
            VirtualTcp {
                incoming: incoming.clone(),
                outgoing: outgoing.clone(),
            },
            VirtualTcp {
                incoming: outgoing,
                outgoing: incoming,
            },
        )
    }
}

impl Connection for VirtualTcp {

}

pub struct CommandReceiver {
    receiver: async_channel::Receiver<Command>,
}

impl CommandReceiver {
    pub fn new() -> (CommandReceiver, async_channel::Sender<Command>) {
        let (sender, receiver) = async_channel::unbounded();
        (CommandReceiver { receiver }, sender)
    }

    pub async fn wait_command(&self) -> Command {
        self.receiver.recv().await.unwrap()
    }
}

pub async fn run<T: Connection>(connections: Vec<T>, command_receiver: CommandReceiver) {
    /*
    use rsa::{PublicKey, RsaPrivateKey, RsaPublicKey, PaddingScheme};
    use rand::rngs::OsRng;

    let mut rng = OsRng; // TODO: Check security
    let bits = 4096;
    let private_key = RsaPrivateKey::new(&mut rng, bits).expect("failed to generate a key");
    let public_key = RsaPublicKey::from(&private_key);
    */

    loop {
        let command = command_receiver.wait_command().await;
        info!("{:?}", command);

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
    use rand::Rng;
    env_logger::init();

    let mut connections: Vec<Vec<VirtualTcp>> = Vec::new();

    for _ in 0..1000 {
        connections.push(Vec::new());
    }

    for i in 0..1000 {
        while connections[i].len() < 8 {
            let j = rand::thread_rng().gen_range(0..1000);
            if j != i && connections[j].len() <= 10 {
                let conn = VirtualTcp::new_channel();
                connections[i].push(conn.0);
                connections[j].push(conn.1);
            }
        }
    }

    println!("All connections made");

    let mut command_senders = Vec::new();
    for connections in connections {
        let (receiver, sender) = CommandReceiver::new();
        command_senders.push(sender);
        tokio::spawn(run(connections, receiver));
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
