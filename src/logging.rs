// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

#[derive(Clone)]
pub struct LogLevel {
    value: std::sync::Arc<std::sync::atomic::AtomicU8>,
}

impl LogLevel {
    pub fn from(value: u8) -> Self {
        Self {
            value: std::sync::Arc::new(std::sync::atomic::AtomicU8::new(value)),
        }
    }
    
    pub fn load(&self) -> u8 {
        self.value.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set(&self, value: u8) {
        self.value.store(value, std::sync::atomic::Ordering::Relaxed)
    }
}

#[macro_export]
macro_rules! error {
    ($i:expr, $($arg:expr),+) => {
        if $i.load() >= 1 {
            log::error!($($arg,)+);
        }
    }
}

#[macro_export]
macro_rules! warn {
    ($i:expr, $($arg:expr),+) => {
        if $i.load() >= 2 {
            log::warn!($($arg,)+);
        }
    }
}

#[macro_export]
macro_rules! info {
    ($i:expr, $($arg:expr),+) => {
        if $i.load() >= 3 {
            log::info!($($arg,)+);
        }
    }
}

#[macro_export]
macro_rules! debug {
    ($i:expr, $($arg:expr),+) => {
        if $i.load() >= 4 {
            log::debug!($($arg,)+);
        }
    }
}

#[macro_export]
macro_rules! trace {
    ($i:expr, $($arg:expr),+) => {
        if $i.load() >= 5 {
            log::trace!($($arg,)+);
        }
    }
}

