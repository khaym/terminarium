//! Deterministic economy engine for an aquarium incremental game.
//!
//! The engine is a pure state machine: no I/O, no clock, no floats. Time is
//! driven from the outside via `State::advance`, which is also how offline
//! progress is settled (settlement == the same simulation).

pub mod engine;
