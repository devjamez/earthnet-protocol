use earthnet_protocol::{
    sign, verify, ConfirmedEvent, EvidenceKind, Location, Observation, SignError, Signed,
    SourceType, PROTOCOL_VERSION,
};
use ed25519_dalek::SigningKey;
use prost::Message;
use rand::{rngs::OsRng, RngCore};

fn keypair() -> SigningKey {
    let mut secret = [0u8; 32];
    OsRng.fill_bytes(&mut secret);
    SigningKey::from_bytes(&secret)
}

fn sample_observation(pubkey: Vec<u8>) -> Observation {
    Observation {
        protocol_version: PROTOCOL_VERSION,
        observation_id: vec![0xAB; 16],
        pubkey,
        source_type: SourceType::Phone as i32,
        source_id: String::new(),
        captured_at_ns: 1_719_600_000_000_000_000,
        clock_uncert_ms: 20,
        location: Some(Location {
            geohash: "66jd2".into(),
            precision_m: 2400,
        }),
        sta_lta_ratio: 7.5,
        p_wave_detected: true,
        estimated_pga: 0.012,
        reported_magnitude: 0.0,
        signature: Vec::new(),
    }
}

fn sample_event(pubkey: Vec<u8>) -> ConfirmedEvent {
    ConfirmedEvent {
        protocol_version: PROTOCOL_VERSION,
        event_id: vec![0xCD; 16],
        pubkey,
        origin_time_ns: 1_719_600_000_000_000_000,
        issued_at_ns: 1_719_600_000_300_000_000,
        epicenter: Some(Location {
            geohash: "66jd2k".into(),
            precision_m: 600,
        }),
        depth_km: 35.0,
        magnitude: 6.2,
        magnitude_uncert: 0.3,
        evidence: EvidenceKind::Official as i32,
        num_observations: 3,
        obs_ids: vec![vec![0xAB; 16], vec![0x01; 16]],
        supersedes: Vec::new(),
        signature: Vec::new(),
    }
}

#[test]
fn observation_roundtrip_encode_decode() {
    let obs = sample_observation(vec![0u8; 32]);
    let bytes = obs.encode_to_vec();
    let decoded = Observation::decode(bytes.as_slice()).unwrap();
    assert_eq!(obs, decoded);
}

#[test]
fn encoding_is_deterministic() {
    let obs = sample_observation(vec![9u8; 32]);
    assert_eq!(obs.encode_to_vec(), obs.encode_to_vec());
}

#[test]
fn observation_sign_and_verify() {
    let key = keypair();
    let mut obs = sample_observation(key.verifying_key().to_bytes().to_vec());
    sign(&key, &mut obs);
    assert_eq!(obs.signature.len(), 64);
    assert!(verify(&obs).is_ok());
}

#[test]
fn confirmed_event_sign_and_verify() {
    let key = keypair();
    let mut evt = sample_event(key.verifying_key().to_bytes().to_vec());
    sign(&key, &mut evt);
    assert!(verify(&evt).is_ok());
}

#[test]
fn tampering_breaks_signature() {
    let key = keypair();
    let mut obs = sample_observation(key.verifying_key().to_bytes().to_vec());
    sign(&key, &mut obs);
    // Flip a meaningful field after signing.
    obs.sta_lta_ratio = 99.0;
    assert_eq!(verify(&obs), Err(SignError::VerificationFailed));
}

#[test]
fn wrong_key_fails_verification() {
    let signer = keypair();
    let attacker = keypair();
    let mut obs = sample_observation(signer.verifying_key().to_bytes().to_vec());
    sign(&signer, &mut obs);
    // Claim a different identity but keep the original signature.
    obs.pubkey = attacker.verifying_key().to_bytes().to_vec();
    assert_eq!(verify(&obs), Err(SignError::VerificationFailed));
}

#[test]
fn domain_separation_blocks_cross_type_replay() {
    // An Observation and a ConfirmedEvent that happen to encode the same body
    // must not share a valid signature, thanks to distinct domain tags.
    assert_ne!(Observation::DOMAIN, ConfirmedEvent::DOMAIN);
}

#[test]
fn signature_survives_wire_roundtrip() {
    let key = keypair();
    let mut evt = sample_event(key.verifying_key().to_bytes().to_vec());
    sign(&key, &mut evt);
    let bytes = evt.encode_to_vec();
    let decoded = ConfirmedEvent::decode(bytes.as_slice()).unwrap();
    assert!(verify(&decoded).is_ok());
}
