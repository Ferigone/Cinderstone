//! Native Java 1.21 FallingBlockEntity state and source lifecycle algorithms.
//!
//! This module owns neither a second Level nor a private entity allocator.
//! Construction requires the actual allocated ID and constructor random owner;
//! world replacement, registration, ticking and tracking stay with the caller.

pub mod persistence;
pub mod state;
pub mod block;
pub mod block_state_codec;
pub mod tick;
pub mod damage;

pub use state::FallingBlockState;
