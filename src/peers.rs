#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PeerID {
    bytes: Box<[u8; 64]>,
}

impl From<&rsa::RsaPublicKey> for PeerID {
    /// `peer_id = sha512(exponent + modulus)` where exponent and modulus bytes are reprensented as little endian
    fn from(key: &rsa::RsaPublicKey) -> Self {
        use rsa::PublicKeyParts;
        use sha2::{Digest, Sha512};
        use std::mem::MaybeUninit;

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
        for our_byte in self.bytes.iter() {
            for their_byte in other.bytes.iter() {
                if our_byte < their_byte {
                    return Some(Ordering::Less);
                } else if our_byte > their_byte {
                    return Some(Ordering::Greater);
                }
            }
        }
        Some(Ordering::Equal)
    }
}

impl Ord for PeerID {
    fn cmp(&self, other: &Self) -> Ordering {
        for our_byte in self.bytes.iter() {
            for their_byte in other.bytes.iter() {
                if our_byte < their_byte {
                    return Ordering::Less;
                } else if our_byte > their_byte {
                    return Ordering::Greater;
                }
            }
        }
        Ordering::Equal
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formatting() {
        let raw_peer_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let peer_id: PeerID = raw_peer_id.parse().unwrap();
        assert_eq!(peer_id.to_string(), raw_peer_id);
    }
}
