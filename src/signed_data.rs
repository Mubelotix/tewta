// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

use protocol::Parcel;
use rsa::{RsaPrivateKey, PaddingScheme, RsaPublicKey, PublicKeyParts, PublicKey};
use crate::{constants::PROTOCOL_SETTINGS, peers::PeerID};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, protocol_derive::Protocol)]
pub struct SignedData<T: Parcel> {
    data: T,
    rsa_public_key_exponent: Vec<u8>,
    rsa_public_key_modulus: Vec<u8>,
    signature: Vec<u8>,
}

impl<T: Parcel> SignedData<T> {
    pub fn verify(&self) -> PeerID {
        let n = rsa::BigUint::from_bytes_le(&self.rsa_public_key_modulus);
        let e = rsa::BigUint::from_bytes_le(&self.rsa_public_key_exponent);
        let rsa_public_key = RsaPublicKey::new(n, e).unwrap();
        // TODO: Error handling in SignedData::verify

        let bytes = self.data.raw_bytes(&PROTOCOL_SETTINGS).unwrap();
        rsa_public_key.verify(PaddingScheme::new_oaep::<Sha256>(), &bytes, &self.signature).unwrap();

        PeerID::from(&rsa_public_key)
    }

    pub fn into_verified(self) -> (PeerID, T) {
        let peer_id = self.verify();
        (peer_id, self.data)
    }
}

pub trait Signable: Parcel {
    fn sign(self, rsa_public_key: RsaPublicKey, rsa_private_key: RsaPrivateKey) -> SignedData<Self>;
}

impl<T> Signable for T where T: Parcel {
    fn sign(self, rsa_public_key: RsaPublicKey, rsa_private_key: RsaPrivateKey) -> SignedData<Self> {

        let bytes = self.raw_bytes(&PROTOCOL_SETTINGS).unwrap();
        // TODO: Error handling in DhtValue::sign

        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let hash = hasher.finalize();

        let signature = rsa_private_key.sign(PaddingScheme::new_oaep::<Sha256>(), hash.as_slice()).unwrap();

        SignedData {
            data: self,
            rsa_public_key_exponent: rsa_public_key.e().to_bytes_le(),
            rsa_public_key_modulus: rsa_public_key.n().to_bytes_le(),
            signature,
        }
    }
}
