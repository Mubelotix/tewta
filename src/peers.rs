use std::mem::MaybeUninit;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PeerID {
    bytes: Box<[u8; 64]>,
}

impl PeerID {
    #[allow(clippy::if_same_then_else)]
    pub fn distance(&self, other: &PeerID) -> Box<[u8; 64]> {
        let (df, dforward) = self.distance_forward(other);
        let (db, dbackward) = self.distance_backward(other);
        if df {
            dbackward
        } else if db {
            dforward
        } else if dforward < dbackward {
            dforward
        } else {
            dbackward
        }
    }

    // Use with caution, will not handle deduction on the first byte
    fn distance_forward(&self, other: &PeerID) -> (bool, Box<[u8; 64]>) {
        unsafe {
            let mut distance: Box<[u8; 64]> = Box::new(MaybeUninit::uninit().assume_init());
            let mut deduction = false;
            for i in 0..64 {
                let (v, o) = other.bytes.get_unchecked(i).overflowing_sub(*self.bytes.get_unchecked(i));
                *distance.get_unchecked_mut(i) = v;
                if o {
                    let mut j = i;
                    loop {
                        if j == 0 {
                            debug_assert!(!deduction);
                            deduction = true;
                            break;
                        }
                        j -= 1;
                        if *distance.get_unchecked(j) > 0 {
                            *distance.get_unchecked_mut(j) -= 1;
                            break;
                        }
                    }
                }
            }
            (deduction, distance)
        }
    }

    fn distance_backward(&self, other: &PeerID) -> (bool, Box<[u8; 64]>) {
        other.distance_forward(self)
    }
}

impl From<&rsa::RsaPublicKey> for PeerID {
    /// `peer_id = sha512(exponent + modulus)` where exponent and modulus bytes are reprensented as little endian
    fn from(key: &rsa::RsaPublicKey) -> Self {
        use rsa::PublicKeyParts;
        use sha2::{Digest, Sha512};

        let mut e = key.e().to_bytes_le();
        let n = key.n().to_bytes_le();
        e.extend(n);
        
        let mut hasher = Sha512::new();
        hasher.update(&e);
        let result = hasher.finalize();

        // uninitialized bytes
        let mut bytes: Box<[u8; 64]> = Box::new(unsafe { MaybeUninit::uninit().assume_init() });
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
        let mut bytes = [0u8; 64];
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
        Ok(Self {bytes: Box::new(<[u8; 64]>::read_field(read, settings, hints)?)})
    }

    fn write_field(&self, write: &mut dyn std::io::Write, settings: &protocol::Settings, hints: &mut protocol::hint::Hints) -> Result<(), protocol::Error> {
        self.bytes.write_field(write, settings, hints)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formatting() {
        let raw_peer_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let peer_id: PeerID = raw_peer_id.parse().unwrap();
        assert_eq!(peer_id.to_string(), raw_peer_id);
    }

    #[test]
    fn test_distance() {
        let raw_peer_id1 = "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000FF00";
        let peer_id1: PeerID = raw_peer_id1.parse().unwrap();
        let raw_peer_id2 = "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000FF01";
        let peer_id2: PeerID = raw_peer_id2.parse().unwrap();
        let distance = peer_id1.distance_forward(&peer_id2);
        assert_eq!(distance.1, Box::new([0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1]));
        
        let raw_peer_id1 = "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000FF00";
        let peer_id1: PeerID = raw_peer_id1.parse().unwrap();
        let raw_peer_id2 = "FF00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000FF01";
        let peer_id2: PeerID = raw_peer_id2.parse().unwrap();
        let distance = peer_id1.distance_forward(&peer_id2);
        assert_eq!(distance.1, Box::new([255,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1]));

        let raw_peer_id1 = "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001";
        let peer_id1: PeerID = raw_peer_id1.parse().unwrap();
        let raw_peer_id2 = "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000100";
        let peer_id2: PeerID = raw_peer_id2.parse().unwrap();
        let distance = peer_id1.distance_forward(&peer_id2);
        assert_eq!(distance.1, Box::new([0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,255]));

        let raw_peer_id1 = "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001";
        let peer_id1: PeerID = raw_peer_id1.parse().unwrap();
        let raw_peer_id2 = "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000100";
        let peer_id2: PeerID = raw_peer_id2.parse().unwrap();
        let distance = peer_id1.distance(&peer_id2);
        assert_eq!(distance, Box::new([0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,255]));

        let raw_peer_id1 = "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000FF";
        let peer_id1: PeerID = raw_peer_id1.parse().unwrap();
        let raw_peer_id2 = "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
        let peer_id2: PeerID = raw_peer_id2.parse().unwrap();
        let distance = peer_id1.distance(&peer_id2);
        assert_eq!(distance, Box::new([0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,255]));
    }
}
