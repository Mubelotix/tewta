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
        cell::UnsafeCell,
    },
    async_mutex::Mutex,
    async_channel::{Sender, Receiver},
    tokio::io::{AsyncWriteExt, AsyncReadExt},
    protocol::{Parcel, Settings as ProtocolSettings},
    log::*,
};

// TODO: remove this
pub type NodeID = u64;
