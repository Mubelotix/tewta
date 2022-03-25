// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

use std::{sync::Arc, io::Write};
use async_channel::{Receiver, Sender};
#[allow(unused_imports)]
use p2pnet::{stream::*, commands::*, node::*, packets::*, peers::*, util::*, logging::*, *};

pub async fn run_node(addr: String, conn_receiver: Receiver<TcpStream>, command_receiver: CommandReceiver, print_command_input: bool) -> Arc<Node> {
    let node = Node::new(addr).await;

    let node2 = Arc::clone(&node);
    tokio::spawn(async move {
        let node = node2;
        loop {
            let stream = conn_receiver.recv().await.unwrap();
            node.on_connection(stream).await;
        }
    });

    let node2 = Arc::clone(&node);
    tokio::spawn(async move {
        let node = node2;
        loop {
            let command = command_receiver.wait_command().await;
            node.on_command(command).await;
    
            // Print command input chars if no command is running anymore
            if unsafe { RUNNING_COMMAND_COUNTER.fetch_sub(1, std::sync::atomic::Ordering::Relaxed) } == 1 && print_command_input {
                print!("\x1b[32m>>> \x1b[0m");
                std::io::stdout().flush().unwrap();
            }
        }
    });

    node
}

/// TODO [#49]: Manage command senders somewhere else as only the simulation needs it

pub async fn launch_network(node_count: usize, print_command_input: bool) -> (Vec<Sender<Command>>, Vec<Arc<Node>>) {
    env_logger::init();
    unsafe {NODE_COUNT.store(node_count, std::sync::atomic::Ordering::Relaxed)};

    let mut command_senders = Vec::new();
    let mut nodes = Vec::new();
    for i in 0..node_count {
        let (command_receiver, command_sender) = CommandReceiver::new();
        command_senders.push(command_sender);
        let (connection_sender, connection_receiver) = async_channel::unbounded();
        LISTENERS.lock().await.push(connection_sender);
        nodes.push(run_node(format!("local-{}", i), connection_receiver, command_receiver, print_command_input).await);
    }

    (command_senders, nodes)
}
