use std::{sync::Arc, io::Write, future::Future, task::Poll, pin::Pin};
use log::*;
use futures::{future::BoxFuture, FutureExt};
use async_mutex::Mutex;
use async_channel::{Sender, Receiver};
use tokio::io::{AsyncRead, AsyncWrite};

mod commands;
use commands::*;

static mut RUNNING_COMMAND_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[cfg(not(feature = "test"))]
type TcpStream = tokio::net::TcpStream;
#[cfg(feature = "test")]
type TcpStream = TestStream;

lazy_static::lazy_static!(
    static ref LISTENERS: Arc<Mutex<Vec<Sender<TcpStream>>>> = Arc::new(Mutex::new(Vec::new()));
);

pub struct TestStream {
    inbound: Arc<Mutex<Vec<u8>>>,
    outbound: Arc<Mutex<Vec<u8>>>,

    inbound_lock_fut: Option<BoxFuture<'static, async_mutex::MutexGuardArc<Vec<u8>>>>,
    outbound_lock_fut: Option<BoxFuture<'static, async_mutex::MutexGuardArc<Vec<u8>>>>,
}

impl TestStream {
    pub fn new() -> (Self, Self) {
        let inbound = Arc::new(Mutex::new(Vec::new()));
        let outbound = Arc::new(Mutex::new(Vec::new()));

        (
            TestStream {
                inbound: inbound.clone(),
                outbound: outbound.clone(),
                inbound_lock_fut: None,
                outbound_lock_fut: None,
            },
            TestStream {
                inbound: outbound,
                outbound: inbound,
                inbound_lock_fut: None,
                outbound_lock_fut: None,
            },
        )
    }
}

impl AsyncRead for TestStream {
    /// WARNING: No notification will be sent when data becomes unavailable.  
    /// This behavior is NOT expected by the trait.
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if self.inbound_lock_fut.is_none() {
            let self_inbound = Arc::clone(&self.inbound);
            self.inbound_lock_fut = Some(async move { self_inbound.lock_arc().await }.boxed());
        }

        if let Poll::Ready(mut inbound) = self.inbound_lock_fut.as_mut().unwrap().as_mut().poll(cx) {
            if buf.remaining() < inbound.len() {
                return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Buffer too small")));
            }
            buf.put_slice(inbound.as_ref());
            inbound.clear();

            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}

impl AsyncWrite for TestStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        if self.outbound_lock_fut.is_none() {
            let self_outbound = Arc::clone(&self.outbound);
            self.outbound_lock_fut = Some(async move { self_outbound.lock_arc().await }.boxed());
        }

        if let Poll::Ready(mut outbound) = self.outbound_lock_fut.as_mut().unwrap().as_mut().poll(cx) {
            outbound.extend_from_slice(buf);
            Poll::Ready(Ok(buf.len()))
        } else {
            Poll::Pending
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Result<(), std::io::Error>> {
        unimplemented!("Shutdown on virtual testing streams is not implemented");
    }
}

// TODO: error handling
#[cfg(feature = "test")]
async fn connect(addr: String) -> TcpStream {
    if !addr.starts_with("local-") {
        panic!("Only local-* addresses are supported for testing");
    }
    
    let (our_stream, their_stream) = TestStream::new();

    LISTENERS.lock().await[addr[7..].parse::<usize>().unwrap()].send(their_stream).await.unwrap();
    
    our_stream
}

#[cfg(not(feature = "test"))]
async fn connect(_addr: String) -> TcpStream {
    unimplemented!()
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
