#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PeerID {
    bytes: Box<[u8; 64]>,
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
