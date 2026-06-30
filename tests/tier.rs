use earthnet_protocol::{
    sign, verify, ConfidenceTier, ConfirmedEvent, EvidenceKind, Location, NodeAttestation,
    PROTOCOL_VERSION,
};
use ed25519_dalek::SigningKey;
use rand::{rngs::OsRng, RngCore};

fn event() -> (SigningKey, ConfirmedEvent) {
    let mut s = [0u8; 32];
    OsRng.fill_bytes(&mut s);
    let key = SigningKey::from_bytes(&s);
    let evt = ConfirmedEvent {
        protocol_version: PROTOCOL_VERSION,
        event_id: vec![1u8; 16],
        pubkey: key.verifying_key().to_bytes().to_vec(),
        origin_time_ns: 1,
        issued_at_ns: 2,
        epicenter: Some(Location {
            geohash: "66jd2".into(),
            precision_m: 600,
        }),
        depth_km: 10.0,
        magnitude: 6.0,
        magnitude_uncert: 0.3,
        evidence: EvidenceKind::Consensus as i32,
        num_observations: 4,
        obs_ids: vec![],
        supersedes: vec![],
        tier: ConfidenceTier::Provisional as i32,
        attestations: vec![],
        signature: vec![],
    };
    (key, evt)
}

#[test]
fn tier_is_part_of_the_signature() {
    let (key, mut e) = event();
    sign(&key, &mut e);
    assert!(verify(&e).is_ok());
    e.tier = ConfidenceTier::Alert as i32; // changing the tier must break the sig
    assert!(verify(&e).is_err());
}

#[test]
fn attestations_do_not_affect_originator_signature() {
    let (key, mut e) = event();
    sign(&key, &mut e);
    assert!(verify(&e).is_ok());
    // appending co-signatures after signing must NOT invalidate the originator's
    // signature — this is what lets PROVISIONAL upgrade to ALERT in flight.
    e.attestations.push(NodeAttestation {
        node_pubkey: vec![7u8; 32],
        signature: vec![8u8; 64],
    });
    e.attestations.push(NodeAttestation {
        node_pubkey: vec![9u8; 32],
        signature: vec![1u8; 64],
    });
    assert!(verify(&e).is_ok());
}
