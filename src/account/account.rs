// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

use crate::prelude::*;

pub struct UserMention {
    pub username: String,
    pub peer_id: PeerID,
    // TODO [$6241fef02a4e9700089fcb03]: Add timestamp in cache
    /// Cached internet address for that peer_id. Might have changed
    pub cached_addr: Option<String>,
    /// Cached addresses for providers of that peer_id. Might have changed
    pub providers_addrs: Vec<(PeerID, String)>,
}

pub enum PropValue {
    // Simple types
    Bool(bool),
    Uint(u64),
    Int(i64),
    Float(f64),
    Date(u64),
    String(String),
    User(UserMention),
    Blob(Vec<u8>),

    // Typed arrays
    BoolArray(Vec<bool>),
    UintArray(Vec<u64>),
    IntArray(Vec<i64>),
    FloatArray(Vec<f64>),
    DateArray(Vec<u64>),
    StringArray(Vec<String>),
    UserArray(Vec<UserMention>),
    BlobArray(Vec<Vec<u8>>),

    // Untyped arrays
    Array(Vec<PropValue>),
    Map(BTreeMap<String, PropValue>),
}

pub struct AccountData {
    pub username: String,
    /// This might not be exhaustive, see the next field for retrieving the total count
    pub followers: Vec<UserMention>,
    pub follower_count: u32,
    /// This might not be exhaustive, see the next field for retrieving the total count
    pub following: Vec<UserMention>,
    pub following_count: u32,
    /// TODO [$6241fef02a4e9700089fcb04]: Doc
    pub backup_peer_id: PeerID,

    /// Optional custom properties that implementations are free to use.
    /// Keys should be prefixed by the implementation name.
    /// 
    /// TODO [$6241fef02a4e9700089fcb05]: Add generic common keys
    pub props: BTreeMap<String, PropValue>,
}


/// An incomplete representation of an account.
pub struct AccountDataSnapshot {

}
