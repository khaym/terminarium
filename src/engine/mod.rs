//! Deterministic aquarium economy. Spec: work/economy-model.md.

mod params;
mod state;

pub use params::{Params, Ratio, ReefKind, Species, MICRO, SLOTS, SPECIES};
pub use state::{Reef, State, ANCHOR_POS_MAX, DEFAULT_ANCHOR_POS};
