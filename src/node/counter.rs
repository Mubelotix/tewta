// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

use std::sync::atomic::AtomicU32;

#[derive(Default)]
pub struct Counter {
    value: AtomicU32,
}

impl Counter {
    pub fn next(&self) -> u32 {
        self.value.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
}
