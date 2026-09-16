//! Compatibility re-export for the generic runtime process capability.
//!
//! The implementation lives in `mncs-model` so source-level host effects and
//! the external process adapter cannot silently diverge.

pub use mncs_model::process::*;
