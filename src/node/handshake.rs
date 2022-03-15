use super::*;

// TODO: HandshakeError should implement Display

#[derive(Debug)]
pub enum HandshakeError {
    UnsupportedVersion,
    UnexpectedPacket,
    InvalidAesKeyLenght,
    InvalidNonce,
    InvalidNonceCopy,
    /// We are connecting to ourselves!
    SamePeer,
    StreamReunitionFailure(tokio::net::tcp::ReuniteError),
}

pub struct HandshakeData {
    pub their_public_key: RsaPublicKey,
    pub their_peer_id: PeerID,
    pub aes_key: AesKey<aes_gcm::aead::generic_array::typenum::U32>,
    pub stream: TcpStream,
}

pub async fn handshake(mut stream: TcpStream, our_peer_id: &PeerID, our_public_key: &RsaPublicKey, our_private_key: &RsaPrivateKey) -> Result<HandshakeData, HandshakeError> {
    // TODO: Handle errors during handshake
    // There are way too many unwraps

    // TODO: Prevent DOS when receiving packets (packet size)

    let (mut r, mut w) = stream.into_split();

    // Send our protocol version
    debug!("Sending protocol version");
    let p = Packet::ProtocolVersion(ProtocolVersionPacket {
        protocol: "p2pnet".to_string(),
        supported_versions: vec![PROTOCOL_VERSION],
    });
    let p = p.raw_bytes(&PROTOCOL_SETTINGS).unwrap();
    let plen = p.len() as u32;
    let mut plen_buf = [0u8; 4];
    plen_buf.copy_from_slice(&plen.to_be_bytes());
    w.write_all(&plen_buf).await.unwrap();
    w.write_all(&p).await.unwrap();

    // Receive their protocol version
    debug!("Receiving protocol version");
    let plen = r.read_u32().await.unwrap();
    let mut p = Vec::with_capacity(plen as usize);
    unsafe {p.set_len(plen as usize)};
    r.read_exact(&mut p).await.unwrap();
    let p = Packet::from_raw_bytes(&p, &PROTOCOL_SETTINGS).unwrap();
    match p {
        Packet::ProtocolVersion(p) => {
            // TODO: We should also accept versions with only the patch version unequal to ours
            if !p.supported_versions.contains(&PROTOCOL_VERSION) {
                warn!("Protocol version not supported");
                return Err(HandshakeError::UnsupportedVersion);
            }
        },
        _ => {
            error!("Expected a protocol version packet");
            return Err(HandshakeError::UnexpectedPacket);
        }
    }

    // Send our RSA public key
    debug!("Sending RSA public key");
    let mut our_nonce = Vec::with_capacity(16);
    unsafe {our_nonce.set_len(16)};
    OsRng.fill(our_nonce.as_mut_slice());
    let p = Packet::InitRsa(InitRsaPacket {
        rsa_public_key_exponent: our_public_key.e().to_bytes_le(),
        rsa_public_key_modulus: our_public_key.n().to_bytes_le(),
        nonce: our_nonce.clone(),
    });
    let p = p.raw_bytes(&PROTOCOL_SETTINGS).unwrap();
    let plen = p.len() as u32;
    let mut plen_buf = [0u8; 4];
    plen_buf.copy_from_slice(&plen.to_be_bytes());
    w.write_all(&plen_buf).await.unwrap();
    w.write_all(&p).await.unwrap();

    // Receive their RSA public key
    debug!("Receiving RSA public key");
    let plen = r.read_u32().await.unwrap();
    let mut p = Vec::with_capacity(plen as usize);
    unsafe {p.set_len(plen as usize)};
    r.read_exact(&mut p).await.unwrap();
    let p = Packet::from_raw_bytes(&p, &PROTOCOL_SETTINGS).unwrap();
    let (their_public_key, their_nonce) = match p {
        Packet::InitRsa(p) => {
            let n = rsa::BigUint::from_bytes_le(&p.rsa_public_key_modulus);
            let e = rsa::BigUint::from_bytes_le(&p.rsa_public_key_exponent);
            if p.nonce.len() != 16 {
                return Err(HandshakeError::InvalidNonce);
            }
            (RsaPublicKey::new(n, e).unwrap(), p.nonce)
        }
        _ => {
            error!("Expected an init rsa packet");
            return Err(HandshakeError::UnexpectedPacket);
        }
    };

    // Stop connecting to ourselves
    let their_peer_id = PeerID::from(&their_public_key);
    if our_peer_id == &their_peer_id {
        return Err(HandshakeError::SamePeer);
    }

    // Send our AES init packet
    debug!("Sending AES init packet");
    let mut our_aes_key_part = Vec::with_capacity(16);
    unsafe {our_aes_key_part.set_len(16)};
    OsRng.fill(our_aes_key_part.as_mut_slice());
    let p = Packet::InitAes(InitAesPacket {
        aes_key_part: our_aes_key_part.clone(),
        nonce: their_nonce,
    });
    let p = p.raw_bytes(&PROTOCOL_SETTINGS).unwrap();
    let p = their_public_key.encrypt(&mut OsRng, PaddingScheme::new_oaep::<sha2::Sha256>(), &p).expect("Failed to encrypt");
    let plen = p.len() as u32;
    let mut plen_buf = [0u8; 4];
    plen_buf.copy_from_slice(&plen.to_be_bytes());
    w.write_all(&plen_buf).await.unwrap();
    w.write_all(&p).await.unwrap();

    // Receive their AES init packet
    debug!("Receiving AES init packet");
    let plen = r.read_u32().await.unwrap();
    let mut p = Vec::with_capacity(plen as usize);
    unsafe {p.set_len(plen as usize)};
    r.read_exact(&mut p).await.unwrap();
    let p = our_private_key.decrypt(PaddingScheme::new_oaep::<sha2::Sha256>(), &p).expect("failed to decrypt");
    let p = Packet::from_raw_bytes(&p, &PROTOCOL_SETTINGS).unwrap();
    let mut their_aes_key_part = match p {
        Packet::InitAes(p) => {
            if p.aes_key_part.len() != 16 {
                return Err(HandshakeError::InvalidAesKeyLenght);
            }

            if p.nonce != our_nonce {
                return Err(HandshakeError::InvalidNonceCopy);
            }

            p.aes_key_part
        },
        _ => {
            return Err(HandshakeError::UnexpectedPacket);
        }
    };

    // Concatenate our and their AES key parts
    let aes_key = match our_peer_id.cmp(&their_peer_id) {
        std::cmp::Ordering::Less => {
            our_aes_key_part.extend(&their_aes_key_part);
            our_aes_key_part
        },
        std::cmp::Ordering::Greater => {
            their_aes_key_part.extend(&our_aes_key_part);
            their_aes_key_part
        },
        std::cmp::Ordering::Equal => {
            return Err(HandshakeError::SamePeer);
        },
    };
    let aes_key = AesKey::clone_from_slice(&aes_key);

    let stream = r.reunite(w).map_err(HandshakeError::StreamReunitionFailure)?;
    Ok(HandshakeData {
        their_public_key,
        their_peer_id,
        aes_key,
        stream,
    })
}
