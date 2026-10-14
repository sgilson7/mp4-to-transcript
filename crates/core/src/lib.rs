//! tool_core: every application rule of MP4 to Transcript.
//!
//! Builder requirement: this crate performs no input or output. It takes
//! values and returns values, so `cargo test` reaches every rule without a
//! browser. `tests/boundary.rs` enforces that, along with the dependency
//! allowlist and the reference ban on floats, hash maps and clocks.
//!
//! The speech models run in the page's worker; what to ask them, and what
//! their answers mean, is decided here.
//!
//! - `api`: the one door the shim calls through, and the screens.
//! - `diarize`: who speaks when, from the two speaker models.
//! - `transcribe`: which stretch the speech model reads next, and which of
//!   its lines are kept.
//! - `transcript`: lines given speakers, numbered, and written in Zoom's layouts.
//! - `bits`: model output read as integers.

pub mod api;
pub mod bits;
pub mod diarize;
pub mod glossary;
pub mod page;
pub mod transcribe;
pub mod transcript;
