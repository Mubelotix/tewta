// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

mod common;
use crate::common::*;
use p2pnet::{commands::*, RUNNING_COMMAND_COUNTER};
use std::io::Write;

#[tokio::main]
async fn main() {
    let mut buf = String::new();
    println!("Node count: ");
    std::io::stdin().read_line(&mut buf).unwrap();
    let node_count = buf.trim().parse::<usize>().unwrap();
    let command_senders = launch_network(node_count, true).await.0;

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
