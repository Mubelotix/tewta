// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

use super::*;
use HandshakeError::*;

// TODO [#13]: HandshakeError should implement Display

#[derive(Debug)]
pub enum HandshakeError {
    UnsupportedVersion,
    UnexpectedPacket,
    InvalidAesKeyLenght,
    InvalidNonce,
    InvalidNonceCopy,
    PacketTooLarge,
    AlreadyConnected,
    SamePeer, // We are connecting to ourselves!
    IdentityMismatch,
    PeerQuitted(QuitPacket),
    ProtocolError(protocol::Error),
    RsaError(rsa::errors::Error),
    IoError(std::io::Error),
    StreamReunitionFailure(tokio::net::tcp::ReuniteError),
}

impl From<std::io::Error> for HandshakeError {
    fn from(e: std::io::Error) -> Self {
        IoError(e)
    }
}

impl From<protocol::Error> for HandshakeError {
    fn from(e: protocol::Error) -> Self {
        ProtocolError(e)
    }
}

impl From<rsa::errors::Error> for HandshakeError {
    fn from(e: rsa::errors::Error) -> Self {
        RsaError(e)
    }
}

impl ToQuit for HandshakeError {
    fn reason_code(&self) -> &'static str {
        match self {
            UnsupportedVersion => "HandshakeError::UnsupportedVersion",
            UnexpectedPacket => "HandshakeError::UnexpectedPacket",
            InvalidAesKeyLenght => "HandshakeError::InvalidAesKeyLenght",
            InvalidNonce => "HandshakeError::InvalidNonce",
            InvalidNonceCopy => "HandshakeError::InvalidNonceCopy",
            PacketTooLarge => "HandshakeError::PacketTooLarge",
            AlreadyConnected => "HandshakeError::AlreadyConnected",
            SamePeer => "HandshakeError::SamePeer",
            IdentityMismatch => "HandshakeError::IdentityMismatch",
            PeerQuitted(_) => "HandshakeError::PeerQuitted",
            ProtocolError(_) => "HandshakeError::ProtocolError",
            RsaError(_) => "HandshakeError::RsaError",
            IoError(_) => "HandshakeError::IoError",
            StreamReunitionFailure(_) => "HandshakeError::StreamReunitionFailure",
        }
    }

    fn message(&self) -> Option<String> { None }

    fn report_fault(&self) -> bool {
        matches!(self, UnexpectedPacket | InvalidNonce | InvalidNonceCopy | ProtocolError(_))
    }
}

impl Node {
    /// Initialize a connection and insert that connection directly
    pub async fn handshake(&self, mut r: ReadHalf, mut w: WriteHalf, expected_peer_id: Option<PeerID>) -> Result<PeerID, HandshakeError> {
        match self.handshake_raw(&mut r, &mut w, expected_peer_id).await {
            Ok((peer_id, addr)) => {
                self.connections.insert(peer_id.clone(), r, w, addr).await.map_err(|_| AlreadyConnected)?;
                Ok(peer_id)
            },
            Err(e) => {
                // Sent quit packet
                let p = Packet::Quit(e.to_quit());
                let p = p.raw_bytes(&PROTOCOL_SETTINGS)?;
                let plen = p.len() as u32;
                let mut plen_buf = [0u8; 4];
                plen_buf.copy_from_slice(&plen.to_be_bytes());
                w.write_all(&plen_buf).await?;
                w.write_all(&p).await?;

                Err(e)
            },
        }
    }

    async fn handshake_raw(&self, r: &mut ReadHalf, w: &mut WriteHalf, expected_peer_id: Option<PeerID>) -> Result<(PeerID, String), HandshakeError> {
        use HandshakeError::*;

        // Send our protocol version
        trace!(self.ll, "Sending protocol version");
        let p = Packet::ProtocolVersion(ProtocolVersionPacket {
            protocol: "p2pnet".to_string(),
            supported_versions: vec![PROTOCOL_VERSION],
        });
        let p = p.raw_bytes(&PROTOCOL_SETTINGS)?;
        let plen = p.len() as u32;
        let mut plen_buf = [0u8; 4];
        plen_buf.copy_from_slice(&plen.to_be_bytes());
        w.write_all(&plen_buf).await?;
        w.write_all(&p).await?;

        // Receive their protocol version
        trace!(self.ll, "Receiving protocol version");
        let plen = r.read_u32().await?;
        if plen >= MAX_PACKET_SIZE {
            return Err(PacketTooLarge);
        }
        let mut p = Vec::with_capacity(plen as usize);
        unsafe {p.set_len(plen as usize)};
        r.read_exact(&mut p).await?;
        let p = Packet::from_raw_bytes(&p, &PROTOCOL_SETTINGS)?;
        match p {
            Packet::ProtocolVersion(p) => {
                // TODO [#16]: We should also accept versions with only the patch version unequal to ours
                if !p.supported_versions.contains(&PROTOCOL_VERSION) {
                    warn!(self.ll, "Protocol version not supported");
                    return Err(UnsupportedVersion);
                }
            },
            Packet::Quit(p) => return Err(PeerQuitted(p)),
            _ => return Err(UnexpectedPacket),
        }

        // Send our RSA public key
        trace!(self.ll, "Sending RSA public key");
        let mut our_nonce = Vec::with_capacity(16);
        unsafe {our_nonce.set_len(16)};
        OsRng.fill(our_nonce.as_mut_slice());
        let p = Packet::InitRsa(InitRsaPacket {
            rsa_public_key_exponent: self.rsa_public_key.e().to_bytes_le(),
            rsa_public_key_modulus: self.rsa_public_key.n().to_bytes_le(),
            nonce: our_nonce.clone(),
        });
        let p = p.raw_bytes(&PROTOCOL_SETTINGS)?;
        let plen = p.len() as u32;
        let mut plen_buf = [0u8; 4];
        plen_buf.copy_from_slice(&plen.to_be_bytes());
        w.write_all(&plen_buf).await?;
        w.write_all(&p).await?;

        // Receive their RSA public key
        trace!(self.ll, "Receiving RSA public key");
        let plen = r.read_u32().await?;
        if plen >= MAX_PACKET_SIZE {
            return Err(PacketTooLarge);
        }
        let mut p = Vec::with_capacity(plen as usize);
        unsafe {p.set_len(plen as usize)};
        r.read_exact(&mut p).await?;
        let p = Packet::from_raw_bytes(&p, &PROTOCOL_SETTINGS)?;
        let (their_public_key, their_nonce) = match p {
            Packet::InitRsa(p) => {
                let n = rsa::BigUint::from_bytes_le(&p.rsa_public_key_modulus);
                let e = rsa::BigUint::from_bytes_le(&p.rsa_public_key_exponent);
                if p.nonce.len() != 16 {
                    return Err(InvalidNonce);
                }
                (RsaPublicKey::new(n, e)?, p.nonce)
            }
            Packet::Quit(p) => return Err(PeerQuitted(p)),
            _ => return Err(UnexpectedPacket),
        };

        // Prevent invalid connections
        let their_peer_id = PeerID::from(&their_public_key);
        if self.peer_id == their_peer_id {
            return Err(SamePeer);
        }
        if let Some(expected_peer_id) = expected_peer_id {
            if expected_peer_id != their_peer_id {
                return Err(IdentityMismatch);
            }
        }
        if self.connections.contains(&their_peer_id).await {
            return Err(AlreadyConnected);
        }

        // Send our AES init packet
        trace!(self.ll, "Sending AES init packet");
        let mut our_aes_key_part = Vec::with_capacity(16);
        unsafe {our_aes_key_part.set_len(16)};
        OsRng.fill(our_aes_key_part.as_mut_slice());
        let p = Packet::InitAes(InitAesPacket {
            aes_key_part: our_aes_key_part.clone(),
            nonce: their_nonce,
        });
        let p = p.raw_bytes(&PROTOCOL_SETTINGS)?;
        #[cfg(not(feature = "no-rsa"))]
        let p = their_public_key.encrypt(&mut OsRng, PaddingScheme::new_oaep::<sha2::Sha256>(), &p)?;
        let plen = p.len() as u32;
        let mut plen_buf = [0u8; 4];
        plen_buf.copy_from_slice(&plen.to_be_bytes());
        w.write_all(&plen_buf).await?;
        w.write_all(&p).await?;

        // Receive their AES init packet
        trace!(self.ll, "Receiving AES init packet");
        let plen = r.read_u32().await?;
        if plen >= MAX_PACKET_SIZE {
            return Err(PacketTooLarge);
        }
        let mut p = Vec::with_capacity(plen as usize);
        unsafe {p.set_len(plen as usize)};
        r.read_exact(&mut p).await?;
        #[cfg(not(feature = "no-rsa"))]
        let p = self.rsa_private_key.decrypt(PaddingScheme::new_oaep::<sha2::Sha256>(), &p)?;
        let p = Packet::from_raw_bytes(&p, &PROTOCOL_SETTINGS)?;
        let mut their_aes_key_part = match p {
            Packet::InitAes(p) => {
                if p.aes_key_part.len() != 16 {
                    return Err(InvalidAesKeyLenght);
                }

                if p.nonce != our_nonce {
                    return Err(InvalidNonceCopy);
                }

                p.aes_key_part
            },
            Packet::Quit(p) => return Err(PeerQuitted(p)),
            _ => return Err(UnexpectedPacket),
        };

        // Concatenate our and their AES key parts
        let aes_key = match self.peer_id.cmp(&their_peer_id) {
            std::cmp::Ordering::Less => {
                our_aes_key_part.extend(&their_aes_key_part);
                our_aes_key_part
            },
            std::cmp::Ordering::Greater => {
                their_aes_key_part.extend(&our_aes_key_part);
                their_aes_key_part
            },
            std::cmp::Ordering::Equal => {
                return Err(SamePeer);
            },
        };
        let aes_key: AesKey<aes_gcm::aead::generic_array::typenum::U32> = AesKey::clone_from_slice(&aes_key);

        // TODO [#27]: Encrypt with AES the next packets

        // Send our Ehlo packet
        trace!(self.ll, "Sending Ehlo packet");
        let p = Packet::Ehlo(EhloPacket {
            addr: self.addr.to_string(),
        });
        let p = p.raw_bytes(&PROTOCOL_SETTINGS)?;
        let plen = p.len() as u32;
        let mut plen_buf = [0u8; 4];
        plen_buf.copy_from_slice(&plen.to_be_bytes());
        w.write_all(&plen_buf).await?;
        w.write_all(&p).await?;

        // Receive their Ehlo packet
        trace!(self.ll, "Receiving their Ehlo packet");
        let plen = r.read_u32().await?;
        if plen >= MAX_PACKET_SIZE {
            return Err(PacketTooLarge);
        }
        let mut p = Vec::with_capacity(plen as usize);
        unsafe {p.set_len(plen as usize)};
        r.read_exact(&mut p).await?;
        let p = Packet::from_raw_bytes(&p, &PROTOCOL_SETTINGS)?;
        let addr = match p {
            Packet::Ehlo(p) => p.addr,
            Packet::Quit(p) => return Err(PeerQuitted(p)),
            _ => return Err(UnexpectedPacket),
        };

        Ok((their_peer_id, addr))
    }
}
