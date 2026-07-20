//! Deterministic aquarium economy. Spec: work/economy-model.md.

mod params;
mod state;

pub use params::{Params, Ratio, RockKind, Species, MICRO, SLOTS, SPECIES};
pub use state::{Rock, State};
