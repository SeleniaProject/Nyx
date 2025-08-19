#![forbid(unsafe_code)]

#[cfg(feature = "hpke")]
mod imp {
    use crate::{Error, Result};
    use hpke::{
        aead::AesGcm128 as HpkeAead,
        kdf::HkdfSha256,
        kem::{Kem, X25519HkdfSha256},
        Deserializable, OpModeR, OpModeS, Serializable,
    };
    use rand::rng_s::OsRng;
    use rand::RngCore;

    /// Sender: encapsulate to recipient'_s public key and encrypt with context
    pub fn seal(pk_recip: &[u8], aad: &[u8], pt: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        type KemType = X25519HkdfSha256;
        let recip_pk = <KemType as Kem>::PublicKey::from_bytes(pk_recip)
            .map_err(|_| Error::Protocol("hpke pk parse".into()))?;
        let mut rng = OsRng;
        let (enc, mut senderctx) = hpke::setup_sender::<HpkeAead, HkdfSha256, KemType, _>(
            &OpModeS::Base,
            &recip_pk,
            b"nyx-hpke",
            &mut rng,
        )
        .map_err(|_| Error::Protocol("hpke setup sender".into()))?;
        let ct = senderctx
            .seal(pt, aad)
            .map_err(|_| Error::Protocol("hpke seal".into()))?;
        Ok((enc.to_byte_s().to_vec(), ct))
    }

    /// Receiver: open ciphertext using encapped key and recipient'_s private key
    pub fn open(sk_recip: &[u8], enc: &[u8], aad: &[u8], ct: &[u8]) -> Result<Vec<u8>> {
        type KemType = X25519HkdfSha256;
        let recip_sk = <KemType as Kem>::PrivateKey::from_bytes(sk_recip)
            .map_err(|_| Error::Protocol("hpke sk parse".into()))?;
        let enc = <KemType as Kem>::EncappedKey::from_bytes(enc)
            .map_err(|_| Error::Protocol("hpke enc parse".into()))?;
        let mut recipctx = hpke::setup_receiver::<HpkeAead, HkdfSha256, KemType>(
            &OpModeR::Base,
            &recip_sk,
            &enc,
            b"nyx-hpke",
        )
        .map_err(|_| Error::Protocol("hpke setup receiver".into()))?;
        let _pt = recipctx
            .open(ct, aad)
            .map_err(|_| Error::Protocol("hpke open".into()))?;
        Ok(pt)
    }

    /// Generate X25519 keypair. Caller must securely store/zeroize the secret key.
    pub fn gen_keypair() -> (Vec<u8>, Vec<u8>) {
        let mut rng = OsRng;
        let (sk, pk) = X25519HkdfSha256::gen_keypair(&mut rng);
        (sk.to_byte_s().to_vec(), pk.to_byte_s().to_vec())
    }

    /// Random AAD helper
    pub fn random_aad(len: usize) -> Vec<u8> {
        let mut rng = OsRng;
        let mut v = vec![0u8; len];
        rng.fill_bytes(&mut v);
        v
    }
}

#[cfg(not(feature = "hpke"))]
mod imp {
    use crate::{Error, Result};
    pub fn seal(_: &[u8], _: &[u8], _: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
        Err(Error::Protocol("hpke feature disabled".into()))
    }
    pub fn open(_: &[u8], _: &[u8], _: &[u8], _: &[u8]) -> Result<Vec<u8>> {
        Err(Error::Protocol("hpke feature disabled".into()))
    }
    pub fn gen_keypair() -> (Vec<u8>, Vec<u8>) {
        (vec![], vec![])
    }
    pub fn random_aad(_: usize) -> Vec<u8> {
        vec![]
    }
}

pub use imp::*;

#[cfg(test)]
mod test_s {
    use super::*;

    #[test]
    fn hpke_roundtrip_when_enabled() -> Result<(), Box<dyn std::error::Error>> {
        // このテストは feature=hpke のときのみ有意
        let (sk, pk) = gen_keypair();
        if pk.is_empty() {
            return Ok(());
        }
        let aad = b"nyx-hpke-aad".to_vec();
        let pt = b"hello hpke".to_vec();
        let (enc, ct) = seal(&pk, &aad, &pt)?;
        let rt = open(&sk, &enc, &aad, &ct)?;
        assert_eq!(rt, pt);
        Ok(())
    }
}
