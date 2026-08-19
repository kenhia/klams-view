//! klams-view as a library, so the integration tests in `tests/` can
//! drive the real `/api` router against a stub klams (#809). The binary
//! (`src/main.rs`) is a thin shell over this.
//!
//! Nothing here is intended as a public API for other crates — the lib
//! target exists because a contract test that reaches through
//! `main.rs` cannot exist, and the contracts this layer absorbs on the
//! UI's behalf deserve a test that sees the same bytes the UI would.

pub mod api;
pub mod config;
pub mod doctor;
pub mod klams;
pub mod metrics;
