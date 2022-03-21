// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

mod channel;
pub use channel::*;
mod errors;
pub use errors::*;
mod parsing;
pub use parsing::*;

use structopt::*;

/// A command that can executed by a user on its node, in order to control and debug it.
/// 
/// When testing, a thousand nodes are running.
/// Prefix the command by the IDs of the node you want to send that command to.
/// For instance, `2-5,7 ping` will send the command `ping` to nodes 2, 3, 4, 5 and 7.
#[derive(StructOpt, Debug, Clone)]
pub enum Command {
    Conns,
    Buckets,
    RefreshBuckets,
    SetLogLevel {
        level: u8,
    },
    Ping {
        node_id: crate::peers::PeerID,
    },
    Store {
        key: crate::peers::KeyID,
        value: String,
    },
    Find {
        key: crate::peers::KeyID,
    },
    Id,
    Add {
        #[structopt(short)]
        interactive: bool,
        #[structopt(short)]
        patch: bool,
        #[structopt(long)]
        files: Vec<String>,
    },
    Fetch {
        #[structopt(long)]
        dry_run: bool,
        #[structopt(long)]
        all: bool,
        repository: Option<String>,
    },
    Commit {
        #[structopt(short)]
        message: Option<String>,
        #[structopt(short)]
        all: bool,
    },
}
