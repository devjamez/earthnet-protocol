//! earthnet-protocol — signed seismic event wire format for EarthNet.
//!
//! The spine of the network: a signed [`Observation`] (raw pick) and a signed
//! [`ConfirmedEvent`] (post-fusion, triggers the alarm). Both are Protobuf
//! messages signed with Ed25519 over a deterministic payload — see [`sign`].

/// Generated Protobuf types (package `earthnet.v1`).
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/earthnet.v1.rs"));
}

pub mod compat;
pub mod detect;
pub mod sign;

pub use proto::{
    ConfirmedEvent, DeviceClass, EvidenceKind, Location, ModalityClass, Observation, PrivacyLevel,
    SeismicPick, Signal, SourceType,
};
pub use sign::{sign, verify, SignError, Signed};

/// Wire protocol major version carried in every v0.1 message's `protocol_version`.
pub const PROTOCOL_VERSION: u32 = 1;

/// Wire protocol major version carried in every v0.2 [`Signal`]'s `protocol_version`.
pub const PROTOCOL_VERSION_V2: u32 = 2;

/// Well-known `schema_id` for the `seismic.pick.v1` payload ([`SeismicPick`]) — the
/// modality the v0.1 [`Observation`] maps to. Real schema_id allocation is
/// content-addressed (a later registry slice); v0.2 starts with this fixed id.
pub const SCHEMA_SEISMIC_PICK_V1: u64 = 1;
