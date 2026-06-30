use earthnet_protocol::compat::{observation_from_signal, signal_from_observation};
use earthnet_protocol::{
    sign, verify, Location, ModalityClass, Observation, SourceType, PROTOCOL_VERSION,
    PROTOCOL_VERSION_V2, SCHEMA_SEISMIC_PICK_V1,
};
use ed25519_dalek::SigningKey;
use rand::{rngs::OsRng, RngCore};

fn sample() -> (SigningKey, Observation) {
    let mut s = [0u8; 32];
    OsRng.fill_bytes(&mut s);
    let key = SigningKey::from_bytes(&s);
    let obs = Observation {
        protocol_version: PROTOCOL_VERSION,
        observation_id: vec![3u8; 16],
        pubkey: key.verifying_key().to_bytes().to_vec(),
        source_type: SourceType::Phone as i32,
        source_id: String::new(),
        captured_at_ns: 1_700_000_000_000_000_000,
        clock_uncert_ms: 10,
        location: Some(Location {
            geohash: "66jd2".into(),
            precision_m: 2400,
        }),
        sta_lta_ratio: 9.0,
        p_wave_detected: true,
        estimated_pga: 0.05,
        reported_magnitude: 0.0,
        signature: Vec::new(),
    };
    (key, obs)
}

#[test]
fn signal_sign_verify_roundtrip() {
    let (key, o) = sample();
    let mut sig = signal_from_observation(&o);
    assert_eq!(sig.protocol_version, PROTOCOL_VERSION_V2);
    assert_eq!(sig.modality_class, ModalityClass::Motion as i32);
    assert_eq!(sig.schema_id, SCHEMA_SEISMIC_PICK_V1);
    sign(&key, &mut sig);
    assert!(verify(&sig).is_ok());
}

#[test]
fn tampered_signal_fails() {
    let (key, o) = sample();
    let mut sig = signal_from_observation(&o);
    sign(&key, &mut sig);
    sig.quality = 0.99; // mutate after signing
    assert!(verify(&sig).is_err());
}

#[test]
fn signal_normalizes_back_to_observation() {
    let (key, o) = sample();
    let mut sig = signal_from_observation(&o);
    sign(&key, &mut sig);
    let back = observation_from_signal(&sig).expect("seismic pick normalizes");
    assert_eq!(back.pubkey, o.pubkey);
    assert_eq!(back.source_type, o.source_type);
    assert_eq!(back.captured_at_ns, o.captured_at_ns);
    assert_eq!(back.sta_lta_ratio, o.sta_lta_ratio);
    assert!(back.p_wave_detected);
    assert_eq!(back.location.unwrap().geohash, "66jd2");
    assert_eq!(back.protocol_version, PROTOCOL_VERSION); // fusion-compatible
}

#[test]
fn non_seismic_signal_is_not_normalized() {
    let (key, o) = sample();
    let mut sig = signal_from_observation(&o);
    sig.schema_id = 999; // unknown schema
    sign(&key, &mut sig);
    assert!(observation_from_signal(&sig).is_none());
}
