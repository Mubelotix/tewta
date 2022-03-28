// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

use crate::prelude::*;

pub trait Hash {
    fn hash(&self) -> Box<[u8; 32]>;
}

impl<T: Parcel> Hash for T {
    fn hash(&self) -> Box<[u8; 32]> {
        use sha2::{Sha256, Digest};

        // TODO: Error handling in Hash
        let bytes = self.raw_bytes(&PROTOCOL_SETTINGS).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(bytes.as_slice());
        let result = hasher.finalize();

        let mut hash: Box<[u8; 32]> = Box::new(unsafe { uninit_array() });
        hash.copy_from_slice(&result);

        hash
    }
}

// TODO: optimize hash implementation for other types
