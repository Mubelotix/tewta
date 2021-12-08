use std::sync::{Arc, Mutex, mpsc};

mod commands;
use commands::*;

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
    receiver: mpsc::Receiver<Command>,
}

impl CommandReceiver {
    pub fn new() -> (CommandReceiver, mpsc::Sender<Command>) {
        let (sender, receiver) = mpsc::channel();
        (CommandReceiver { receiver }, sender)
    }

    pub fn wait_command(&self) -> Command {
        self.receiver.recv().unwrap() // TODO: handle?
    }
}

pub async fn run<T: Connection>(connections: Vec<T>, command_receiver: CommandReceiver) {
    use rsa::{PublicKey, RsaPrivateKey, RsaPublicKey, PaddingScheme};
    use rand::rngs::OsRng;

    let mut rng = OsRng; // TODO: Check security
    let bits = 4096;
    let private_key = RsaPrivateKey::new(&mut rng, bits).expect("failed to generate a key");
    let public_key = RsaPublicKey::from(&private_key);

    loop {
        let command = command_receiver.wait_command();
        match command {
            Command::DebugState => {
                println!("I have {} connections", connections.len());
            },
            Command::Unparsable(test) => {
                println!("Unparsable command: {}", test);
            },
        }
    }
}

fn main() {
    println!("Hello, world!");
}

#[tokio::test]
async fn thousand_nodes() {
    use rand::Rng;

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
        tokio::spawn(async move { run(connections, receiver).await });
    }

    println!("All nodes running");

    loop {
        let mut raw_command = String::new();
        std::io::stdin().read_line(&mut raw_command).unwrap();
        match Command::parse(&raw_command) {
            Ok((destinators, command)) => {
                for destinator in destinators {
                    command_senders[destinator].send(command.clone()).unwrap();
                }
            }
            Err(e) => println!("{}", e),
        }
    }
}
