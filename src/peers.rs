// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

#![allow(clippy::needless_range_loop)]

use crate::prelude::*;
use crate::util::uninit_array;
use std::hint::unreachable_unchecked;

pub type KeyID = PeerID;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PeerID {
    bytes: Box<[u8; 32]>,
}

impl PeerID {
    pub fn distance(&self, other: &PeerID) -> Box<[u8; 32]> {
        // TODO [#32]: Add more safety to this
        // By using MaybeUninit (see connections.rs in buckets)
        // Note: there is another place to improve below
        let mut distance: Box<[u8; 32]> = Box::new(unsafe { uninit_array() });
        for i in 0..32 {
            distance[i] = self.bytes[i] ^ other.bytes[i];
        }
        distance
    }

    pub fn bytes(&self) -> &[u8; 32] {
        &self.bytes
    }

    pub fn matches(&self, other: &PeerID, mask: &[u8]) -> bool {
        debug_assert!(mask.len() <= 32);
        for i in 0..mask.len() {
            if self.bytes[i] & mask[i] != other.bytes[i] & mask[i] {
                return false;
            }
        }
        true
    }

    pub fn bucket(&self, other: &PeerID) -> Option<(usize, usize)> {
        for i in 0..32 {
            let distance = self.bytes[i] ^ other.bytes[i];
            // TODO [#33]: Check that this optimization is really useful
            if distance == 0 {
                continue;
            }
            match distance & 0b11000000 {
                0 => (),
                0b01000000 => return Some((4*i, 0)),
                0b10000000 => return Some((4*i, 1)),
                0b11000000 => return Some((4*i, 2)),
                _ => unreachable!(),
            }
            match distance & 0b00110000 {
                0 => (),
                0b00010000 => return Some((4*i+1, 0)),
                0b00100000 => return Some((4*i+1, 1)),
                0b00110000 => return Some((4*i+1, 2)),
                _ => unreachable!(),
            }
            match distance & 0b00001100 {
                0 => (),
                0b00000100 => return Some((4*i+2, 0)),
                0b00001000 => return Some((4*i+2, 1)),
                0b00001100 => return Some((4*i+2, 2)),
                _ => unreachable!(),
            }
            match distance & 0b00000011 {
                0 => (),
                0b00000001 => return Some((4*i+3, 0)),
                0b00000010 => return Some((4*i+3, 1)),
                0b00000011 => return Some((4*i+3, 2)),
                _ => unreachable!(),
            }
        }
        None
    }

    /// Generates `target` such as `self.bucket(target) = Some((bucket_level, bucket_id))`
    pub fn generate_in_bucket(&self, bucket_level: usize, bucket_id: usize) -> PeerID {
        assert!(bucket_level < 128 && bucket_id < 3);
        let mut target = Box::new(unsafe { uninit_array() });
        let i = bucket_level.div_euclid(4); // (i < 128.div_euclid(4) < 32) => i <= 31
        target[..i].clone_from_slice(&self.bytes[..i]);
        match bucket_level.rem_euclid(4) {
            0 => {
                match bucket_id {
                    0 => target[i] = self.bytes[i] ^ 0b01000000,
                    1 => target[i] = self.bytes[i] ^ 0b10000000,
                    2 => target[i] = self.bytes[i] ^ 0b11000000,
                    _ => unsafe { unreachable_unchecked() },
                }
            }
            1 => {
                match bucket_id {
                    0 => target[i] = self.bytes[i] ^ 0b00010000,
                    1 => target[i] = self.bytes[i] ^ 0b00100000,
                    2 => target[i] = self.bytes[i] ^ 0b00110000,
                    _ => unsafe { unreachable_unchecked() },
                }
            }
            2 => {
                match bucket_id {
                    0 => target[i] = self.bytes[i] ^ 0b00000100,
                    1 => target[i] = self.bytes[i] ^ 0b00001000,
                    2 => target[i] = self.bytes[i] ^ 0b00001100,
                    _ => unsafe { unreachable_unchecked() },
                }
            }
            3 => {
                match bucket_id {
                    0 => target[i] = self.bytes[i] ^ 0b00000001,
                    1 => target[i] = self.bytes[i] ^ 0b00000010,
                    2 => target[i] = self.bytes[i] ^ 0b00000011,
                    _ => unsafe { unreachable_unchecked() },
                }
            }
            _ => unsafe { unreachable_unchecked() },
        }

        unsafe {
            target.get_unchecked_mut(i+1..32).copy_from_slice(self.bytes.get_unchecked(i+1..32));
        }

        PeerID { bytes: target }
    }
}

impl From<&rsa::RsaPublicKey> for PeerID {
    /// `peer_id = sha512(exponent + modulus)` where exponent and modulus bytes are reprensented as little endian
    fn from(key: &rsa::RsaPublicKey) -> Self {
        let mut e = key.e().to_bytes_le();
        let n = key.n().to_bytes_le();
        e.extend(n);
        
        let mut hasher = Sha256::new();
        hasher.update(&e);
        let result = hasher.finalize();

        // uninitialized bytes
        let mut bytes: Box<[u8; 32]> = Box::new(unsafe { uninit_array() });
        bytes.copy_from_slice(&result);

        PeerID { bytes }
    }
}

use std::cmp::{Ord, PartialOrd, Ordering};
impl PartialOrd<PeerID> for PeerID {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.bytes.partial_cmp(&other.bytes)
    }
}

impl Ord for PeerID {
    fn cmp(&self, other: &Self) -> Ordering {
        self.bytes.cmp(&other.bytes)
    }
}

impl std::fmt::Display for PeerID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.bytes.iter() {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

impl std::str::FromStr for PeerID {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 32];
        for (i, byte) in bytes.iter_mut().enumerate() {
            let s = s.get(i*2..i*2+2).unwrap_or("");
            *byte = u8::from_str_radix(s, 16)?;
        }
        Ok(PeerID { bytes: bytes.into() })
    }
}

impl protocol::Parcel for PeerID {
    const TYPE_NAME: &'static str = "PeerID";

    fn read_field(read: &mut dyn std::io::Read, settings: &protocol::Settings, hints: &mut protocol::hint::Hints) -> Result<Self, protocol::Error> {
        Ok(Self {bytes: Box::new(<[u8; 32]>::read_field(read, settings, hints)?)})
    }

    fn write_field(&self, write: &mut dyn std::io::Write, settings: &protocol::Settings, hints: &mut protocol::hint::Hints) -> Result<(), protocol::Error> {
        self.bytes.write_field(write, settings, hints)
    }
}

impl Hashable for PeerID {
    fn update_hasher(&self, hasher: &mut impl Digest) {
        hasher.update(self.bytes.as_ref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formatting() {
        let raw_peer_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let peer_id: PeerID = raw_peer_id.parse().unwrap();
        assert_eq!(peer_id.to_string(), raw_peer_id);
    }

    #[test]
    fn test_distance() {
        let raw_peer_id1 = "FF00000000000000000000000000000000000000000000000000000000000001";
        let peer_id1: PeerID = raw_peer_id1.parse().unwrap();
        let raw_peer_id2 = "FF00000000000000000000000000000000000000000000000000000000000100";
        let peer_id2: PeerID = raw_peer_id2.parse().unwrap();
        let distance = peer_id1.distance(&peer_id2);
        assert_eq!(distance, Box::new([0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,1]));

        let raw_peer_id1 = "00000000000000000000000000000000000000000000000000000000000000FF";
        let peer_id1: PeerID = raw_peer_id1.parse().unwrap();
        let raw_peer_id2 = "0000000000000000000000000000000000000000000000000000000000000000";
        let peer_id2: PeerID = raw_peer_id2.parse().unwrap();
        let distance = peer_id1.distance(&peer_id2);
        assert_eq!(distance, Box::new([0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,255]));
    }

    #[test]
    fn test_bucket_level() {
        let raw_peer_id1 = "0000000000000000000000000000000000000000000000000000000000000000";
        let peer_id1: PeerID = raw_peer_id1.parse().unwrap();
        let raw_peer_id2 = "0000000000000000000000000000000000000000000000000000000000000000";
        let peer_id2: PeerID = raw_peer_id2.parse().unwrap();
        assert_eq!(peer_id1.bucket(&peer_id2), None);

        let raw_peer_id1 = "0000000000000000000000000000000000000000000000000000000000000000";
        let peer_id1: PeerID = raw_peer_id1.parse().unwrap();
        let raw_peer_id2 = "F000000000000000000000000000000000000000000000000000000000000000";
        let peer_id2: PeerID = raw_peer_id2.parse().unwrap();
        assert_eq!(peer_id1.bucket(&peer_id2).unwrap().0, 0);

        let raw_peer_id1 = "0000000000000000000000000000000000000000000000000000000000000000";
        let peer_id1: PeerID = raw_peer_id1.parse().unwrap();
        let raw_peer_id2 = "3000000000000000000000000000000000000000000000000000000000000000";
        let peer_id2: PeerID = raw_peer_id2.parse().unwrap();
        assert_eq!(peer_id1.bucket(&peer_id2).unwrap().0, 1);

        let raw_peer_id1 = "0000000000000000000000000000000000000000000000000000000000000000";
        let peer_id1: PeerID = raw_peer_id1.parse().unwrap();
        let raw_peer_id2 = "0F00000000000000000000000000000000000000000000000000000000000000";
        let peer_id2: PeerID = raw_peer_id2.parse().unwrap();
        assert_eq!(peer_id1.bucket(&peer_id2).unwrap().0, 2);

        let raw_peer_id1 = "00F0000000000000000000000000000000000000000000000000000000000000";
        let peer_id1: PeerID = raw_peer_id1.parse().unwrap();
        let raw_peer_id2 = "00F000000000000000000000000000000000000000000000000000000000000E";
        let peer_id2: PeerID = raw_peer_id2.parse().unwrap();
        assert_eq!(peer_id1.bucket(&peer_id2).unwrap().0, 126);
    }

    #[test]
    fn test_generate_in_bucket() {
        let raw_peer_id1 = "0000000000000000000000000000000000000000000000000000000000000000";
        let peer_id: PeerID = raw_peer_id1.parse().unwrap();

        for bucket_level in 0..128 {
            for bucket_id in 0..3 {
                let peer_id2 = peer_id.generate_in_bucket(bucket_level, bucket_id);
                assert_eq!(peer_id.bucket(&peer_id2), Some((bucket_level, bucket_id)));
            }
        }
    }
}
