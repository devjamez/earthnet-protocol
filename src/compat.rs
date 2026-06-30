//! v0.1 ⇄ v0.2 conversions for dual-stack operation.
//!
//! The node runs one fusion engine over v0.1 [`Observation`]s. A v0.2 [`Signal`]
//! carrying a seismic pick is **normalized** into that internal `Observation`, so
//! consensus/locate/magnitude stay unchanged. Other modalities don't feed the
//! seismic alert path (yet) and return `None`.

use prost::Message;

use crate::{
    ModalityClass, Observation, SeismicPick, Signal, PROTOCOL_VERSION, PROTOCOL_VERSION_V2,
    SCHEMA_SEISMIC_PICK_V1,
};

/// Encodes the `seismic.pick.v1` payload from a v0.1 Observation's pick fields.
pub fn seismic_pick_payload(obs: &Observation) -> Vec<u8> {
    SeismicPick {
        sta_lta_ratio: obs.sta_lta_ratio,
        p_wave_detected: obs.p_wave_detected,
        estimated_pga: obs.estimated_pga,
        reported_magnitude: obs.reported_magnitude,
    }
    .encode_to_vec()
}

/// Builds an **unsigned** v0.2 `Signal{MOTION, seismic.pick.v1}` from a v0.1
/// Observation. The caller must sign it — the Signal signature (domain
/// `earthnet-signal-v2`) differs from the Observation's.
pub fn signal_from_observation(obs: &Observation) -> Signal {
    Signal {
        protocol_version: PROTOCOL_VERSION_V2,
        observation_id: obs.observation_id.clone(),
        pubkey: obs.pubkey.clone(),
        modality_class: ModalityClass::Motion as i32,
        schema_id: SCHEMA_SEISMIC_PICK_V1,
        source_type: obs.source_type,
        device_class: 0,
        location: obs.location.clone(),
        captured_at_ns: obs.captured_at_ns,
        clock_uncert_ms: obs.clock_uncert_ms,
        quality: 0.0,
        payload: seismic_pick_payload(obs),
        signature: Vec::new(),
    }
}

/// Normalizes a v0.2 Signal that carries a seismic pick into the internal v0.1
/// Observation the fusion engine consumes. Returns `None` if it is not a seismic
/// pick. The returned Observation has **no signature** (the Signal was verified
/// upstream; fusion does not re-verify).
pub fn observation_from_signal(sig: &Signal) -> Option<Observation> {
    if sig.modality_class != ModalityClass::Motion as i32 || sig.schema_id != SCHEMA_SEISMIC_PICK_V1
    {
        return None;
    }
    let pick = SeismicPick::decode(sig.payload.as_slice()).ok()?;
    Some(Observation {
        protocol_version: PROTOCOL_VERSION,
        observation_id: sig.observation_id.clone(),
        pubkey: sig.pubkey.clone(),
        source_type: sig.source_type,
        source_id: String::new(),
        captured_at_ns: sig.captured_at_ns,
        clock_uncert_ms: sig.clock_uncert_ms,
        location: sig.location.clone(),
        sta_lta_ratio: pick.sta_lta_ratio,
        p_wave_detected: pick.p_wave_detected,
        estimated_pga: pick.estimated_pga,
        reported_magnitude: pick.reported_magnitude,
        signature: Vec::new(),
    })
}
