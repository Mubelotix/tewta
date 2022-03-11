mod connections;
pub use connections::*;
mod events;
pub use events::*;
mod node;
pub use node::*;

pub(self) use {
    crate::{
        commands::Command,
        stream::TcpStream,
        packets::*,
        connect,
    },
    std::{
        sync::{Arc, Weak},
        collections::HashMap,
    },
    async_mutex::Mutex,
    async_channel::{Sender, Receiver},
    tokio::io::AsyncWriteExt,
    protocol::Parcel,
    log::*,
};

// TODO: remove this
type NodeID = u64;
