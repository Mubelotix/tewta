mod connections;
pub use connections::*;
mod events;
pub use events::*;
mod node;
pub use node::*;
mod counter;
pub use counter::*;

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
        time::{Duration, Instant},
        default::Default,
    },
    async_mutex::Mutex,
    async_channel::{Sender, Receiver},
    tokio::{
        io::{AsyncWriteExt, AsyncReadExt},
        time::timeout,
    },
    protocol::{Parcel, Settings as ProtocolSettings},
    log::*,
};

// TODO [#5]: remove this
pub type NodeID = u64;
