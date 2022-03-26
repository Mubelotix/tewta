// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

use crate::prelude::*;

#[derive(Debug, Clone, Protocol)]
pub struct AccountSnapshotDescriptor {
    timestamp: u64,
    hash: Vec<u8>,
}
