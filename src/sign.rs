//! Ed25519 signing over a deterministic protobuf payload.
//!
//! Signing scheme (normative, see PROTOCOL-v0.1-DRAFT.md):
//! ```text
//! signing_payload = domain_tag || deterministic_encode(message with signature = empty)
//! signature       = Ed25519_sign(privkey, signing_payload)
//! ```
//! The domain tag provides cross-message replay separation. The signature field is
//! cleared before encoding so it never signs over itself.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use prost::Message;

/// Domain-separation tag for [`Observation`](crate::Observation).
pub const DOMAIN_OBSERVATION: &[u8] = b"earthnet-obs-v1";
/// Domain-separation tag for [`ConfirmedEvent`](crate::ConfirmedEvent).
pub const DOMAIN_CONFIRMED_EVENT: &[u8] = b"earthnet-evt-v1";
/// Domain-separation tag for the v0.2 [`Signal`](crate::Signal) envelope.
pub const DOMAIN_SIGNAL: &[u8] = b"earthnet-signal-v2";

/// A protobuf message that carries an Ed25519 identity + signature and can be
/// signed/verified with the EarthNet scheme.
pub trait Signed: Message + Clone {
    /// Domain-separation tag prepended to the signing payload.
    const DOMAIN: &'static [u8];
    /// Raw bytes of the signer's Ed25519 public key (expected 32 bytes).
    fn pubkey(&self) -> &[u8];
    /// Raw bytes of the signature (expected 64 bytes).
    fn signature(&self) -> &[u8];
    /// Overwrite the signature field.
    fn set_signature(&mut self, sig: Vec<u8>);
}

/// Errors from [`verify`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignError {
    /// Public key was not a valid 32-byte Ed25519 key.
    BadPublicKey,
    /// Signature was not a valid 64-byte Ed25519 signature.
    BadSignature,
    /// Signature did not verify against the payload.
    VerificationFailed,
}

impl core::fmt::Display for SignError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            SignError::BadPublicKey => "invalid Ed25519 public key",
            SignError::BadSignature => "invalid Ed25519 signature encoding",
            SignError::VerificationFailed => "signature verification failed",
        };
        f.write_str(s)
    }
}

impl std::error::Error for SignError {}

/// Builds `domain || encode(msg)`. Caller must clear the signature field first.
fn signing_payload<T: Signed>(msg: &T) -> Vec<u8> {
    let mut buf = Vec::with_capacity(T::DOMAIN.len() + msg.encoded_len());
    buf.extend_from_slice(T::DOMAIN);
    // Encoding into a Vec is infallible in prost.
    msg.encode(&mut buf)
        .expect("prost encode into Vec is infallible");
    buf
}

/// Signs `msg` in place: clears the signature field, computes the payload, and
/// writes the resulting 64-byte signature back into the message.
pub fn sign<T: Signed>(key: &SigningKey, msg: &mut T) {
    msg.set_signature(Vec::new());
    let payload = signing_payload(msg);
    let sig = key.sign(&payload);
    msg.set_signature(sig.to_bytes().to_vec());
}

/// Verifies the signature carried in `msg` against the pubkey carried in `msg`.
pub fn verify<T: Signed>(msg: &T) -> Result<(), SignError> {
    let pk: [u8; 32] = msg
        .pubkey()
        .try_into()
        .map_err(|_| SignError::BadPublicKey)?;
    let vk = VerifyingKey::from_bytes(&pk).map_err(|_| SignError::BadPublicKey)?;

    let sig_bytes: [u8; 64] = msg
        .signature()
        .try_into()
        .map_err(|_| SignError::BadSignature)?;
    let sig = Signature::from_bytes(&sig_bytes);

    let mut bare = msg.clone();
    bare.set_signature(Vec::new());
    let payload = signing_payload(&bare);

    vk.verify(&payload, &sig)
        .map_err(|_| SignError::VerificationFailed)
}

impl Signed for crate::Observation {
    const DOMAIN: &'static [u8] = DOMAIN_OBSERVATION;
    fn pubkey(&self) -> &[u8] {
        &self.pubkey
    }
    fn signature(&self) -> &[u8] {
        &self.signature
    }
    fn set_signature(&mut self, sig: Vec<u8>) {
        self.signature = sig;
    }
}

impl Signed for crate::ConfirmedEvent {
    const DOMAIN: &'static [u8] = DOMAIN_CONFIRMED_EVENT;
    fn pubkey(&self) -> &[u8] {
        &self.pubkey
    }
    fn signature(&self) -> &[u8] {
        &self.signature
    }
    fn set_signature(&mut self, sig: Vec<u8>) {
        self.signature = sig;
    }
}

impl Signed for crate::Signal {
    const DOMAIN: &'static [u8] = DOMAIN_SIGNAL;
    fn pubkey(&self) -> &[u8] {
        &self.pubkey
    }
    fn signature(&self) -> &[u8] {
        &self.signature
    }
    fn set_signature(&mut self, sig: Vec<u8>) {
        self.signature = sig;
    }
}
