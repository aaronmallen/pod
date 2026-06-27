//! Telemetry support.
//!
//! This module owns the pure data-handling pieces of the opt-in telemetry
//! pipeline. Today that is [`pii`], a side-effect-free scrubber that enforces
//! the never-collected boundary on crash content and buffered log lines before
//! anything is allowed to leave the process.

pub mod pii;
