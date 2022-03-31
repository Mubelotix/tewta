// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

use crate::prelude::*;

pub trait Hash {
    fn hash(&self) -> Box<[u8; 32]>;
}

impl<T: Hashable> Hash for T {
    fn hash(&self) -> Box<[u8; 32]> {
        use sha2::{Sha256, Digest};

        let mut hasher = Sha256::new();
        self.update_hasher(&mut hasher);
        let result = hasher.finalize();

        let mut hash = Box::new(unsafe {uninit_array()});
        hash.copy_from_slice(&result);

        hash
    }
}
