use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use p256::{
    ecdsa::{signature::Verifier, DerSignature, VerifyingKey},
    pkcs8::DecodePublicKey,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SignatureVerificationError {
    #[error("public key is not valid base64")]
    InvalidPublicKeyBase64,
    #[error("public key is not a P-256 SPKI key")]
    InvalidPublicKey,
    #[error("signature is not valid base64")]
    InvalidSignatureBase64,
    #[error("signature is not a DER-encoded P-256 signature")]
    InvalidSignature,
    #[error("signature verification failed")]
    VerificationFailed,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn validate_p256_public_key(
    public_key_spki_base64: &str,
) -> Result<(), SignatureVerificationError> {
    let public_key_der = BASE64_STANDARD
        .decode(public_key_spki_base64)
        .map_err(|_| SignatureVerificationError::InvalidPublicKeyBase64)?;
    VerifyingKey::from_public_key_der(&public_key_der)
        .map(|_| ())
        .map_err(|_| SignatureVerificationError::InvalidPublicKey)
}

pub fn verify_p256_signature(
    public_key_spki_base64: &str,
    signature_der_base64: &str,
    message: &[u8],
) -> Result<(), SignatureVerificationError> {
    let public_key_der = BASE64_STANDARD
        .decode(public_key_spki_base64)
        .map_err(|_| SignatureVerificationError::InvalidPublicKeyBase64)?;
    let verifying_key = VerifyingKey::from_public_key_der(&public_key_der)
        .map_err(|_| SignatureVerificationError::InvalidPublicKey)?;

    let signature_der = BASE64_STANDARD
        .decode(signature_der_base64)
        .map_err(|_| SignatureVerificationError::InvalidSignatureBase64)?;
    let signature = DerSignature::from_bytes(&signature_der)
        .map_err(|_| SignatureVerificationError::InvalidSignature)?;

    verifying_key
        .verify(message, &signature)
        .map_err(|_| SignatureVerificationError::VerificationFailed)
}

#[cfg(test)]
mod tests {
    use p256::{
        ecdsa::{signature::Signer, Signature, SigningKey},
        elliptic_curve::rand_core::OsRng,
        pkcs8::EncodePublicKey,
    };

    use super::*;

    fn signed_message(message: &[u8]) -> (String, String) {
        let signing_key = SigningKey::random(&mut OsRng);
        let verifying_key = VerifyingKey::from(&signing_key);
        let public_key_der = verifying_key.to_public_key_der().unwrap();
        let signature: Signature = signing_key.sign(message);

        (
            BASE64_STANDARD.encode(public_key_der.as_bytes()),
            BASE64_STANDARD.encode(signature.to_der().as_bytes()),
        )
    }

    #[test]
    fn verifies_android_compatible_spki_and_der_signature() {
        let message = b"BRIEFLI-SYNC-V1\nPUT\n/v1/captures/example";
        let (public_key, signature) = signed_message(message);

        assert_eq!(
            verify_p256_signature(&public_key, &signature, message),
            Ok(())
        );
    }

    #[test]
    fn rejects_tampered_message() {
        let (public_key, signature) = signed_message(b"original");

        let error = verify_p256_signature(&public_key, &signature, b"tampered")
            .unwrap_err();

        assert_eq!(error, SignatureVerificationError::VerificationFailed);
    }

    #[test]
    fn rejects_signature_from_different_device_key() {
        let message = b"same message";
        let (public_key, _) = signed_message(message);
        let (_, other_signature) = signed_message(message);

        let error = verify_p256_signature(&public_key, &other_signature, message)
            .unwrap_err();

        assert_eq!(error, SignatureVerificationError::VerificationFailed);
    }

    #[test]
    fn rejects_malformed_key_and_signature() {
        assert_eq!(
            verify_p256_signature("not-base64!", "not-base64!", b"message"),
            Err(SignatureVerificationError::InvalidPublicKeyBase64)
        );

        let (public_key, _) = signed_message(b"message");
        assert_eq!(
            verify_p256_signature(&public_key, "not-base64!", b"message"),
            Err(SignatureVerificationError::InvalidSignatureBase64)
        );
        assert_eq!(
            validate_p256_public_key(&BASE64_STANDARD.encode(b"not an SPKI key")),
            Err(SignatureVerificationError::InvalidPublicKey)
        );
    }

    #[test]
    fn sha256_hex_matches_known_vector() {
        assert_eq!(
            sha256_hex(b"briefli"),
            "313adda4407753d970adfbf3aec27b0475430b08e6e6477b054301ea3ba7199c"
        );
    }
}