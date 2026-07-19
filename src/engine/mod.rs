//! Deterministic aquarium economy. Spec: work/economy-model.md.

mod params;
mod state;

pub use params::{Params, Ratio, Species, MICRO, SPECIES};
pub use state::State;
