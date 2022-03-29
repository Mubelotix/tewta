// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

use sha2::{Digest, Sha256};
use rsa::{RsaPrivateKey, PaddingScheme, RsaPublicKey, PublicKeyParts, PublicKey};
use crate::prelude::*;

#[derive(Debug, Clone, protocol_derive::Protocol)]
pub struct SignedData<T: Parcel> {
    data: T,
    rsa_public_key_exponent: Vec<u8>,
    rsa_public_key_modulus: Vec<u8>,
    signature: Vec<u8>,
}

impl<T: Parcel> SignedData<T> {
    pub fn verify(&self) -> Result<PeerID, rsa::errors::Error> {
        let n = rsa::BigUint::from_bytes_le(&self.rsa_public_key_modulus);
        let e = rsa::BigUint::from_bytes_le(&self.rsa_public_key_exponent);
        let rsa_public_key = RsaPublicKey::new(n, e)?;

        // TODO: we shouldn't hash this way because it makes SegmentedArray useless
        let bytes = self.data.raw_bytes(&PROTOCOL_SETTINGS).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let hash = hasher.finalize();

        rsa_public_key.verify(PaddingScheme::new_pss::<Sha256, OsRng>(OsRng), &hash, &self.signature)?;

        Ok(PeerID::from(&rsa_public_key))
    }

    pub fn into_verified(self) -> Result<(PeerID, T), rsa::errors::Error> {
        let peer_id = self.verify()?;
        Ok((peer_id, self.data))
    }
}

pub trait Signable: Parcel {
    fn sign(self, rsa_public_key: &RsaPublicKey, rsa_private_key: &RsaPrivateKey) -> Result<SignedData<Self>, rsa::errors::Error>;
}

impl<T> Signable for T where T: Parcel {
    fn sign(self, rsa_public_key: &RsaPublicKey, rsa_private_key: &RsaPrivateKey) -> Result<SignedData<Self>, rsa::errors::Error> {
        let bytes = self.raw_bytes(&PROTOCOL_SETTINGS).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let hash = hasher.finalize();

        // TODO: Investigate security implications of the PSS padding scheme
        // Can we just ignore the salt lenght?
        let signature = rsa_private_key.sign(PaddingScheme::new_pss::<Sha256, OsRng>(OsRng), hash.as_slice())?;

        Ok(SignedData {
            data: self,
            rsa_public_key_exponent: rsa_public_key.e().to_bytes_le(),
            rsa_public_key_modulus: rsa_public_key.n().to_bytes_le(),
            signature,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        // Generate rsa keys 
        let private_key = RsaPrivateKey::new(&mut OsRng, 512).expect("failed to generate a key");
        let public_key = RsaPublicKey::from(&private_key);
        let peer_id = PeerID::from(&public_key);

        // Sign data
        let data = Packet::Ping(PingPacket { ping_id: 0 });
        let signed_data = data.sign(&public_key, &private_key).unwrap();
        
        // Verify data
        let verified_peer_id = signed_data.verify().unwrap();
        assert_eq!(peer_id, verified_peer_id);
    }
}
