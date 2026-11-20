//! Read-only GRACE 5 artifact parsing.
//!
//! The grace5 kernel remains authoritative; this module only reads `.grace5/`
//! and never writes to it.

pub mod model;
pub mod parse;