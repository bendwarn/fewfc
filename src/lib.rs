//! Deterministic rules engine for CFECards.
//!
//! The crate is intentionally split by Clean Architecture boundaries:
//! domain types, application orchestration, rule registries, ports, and
//! infrastructure adapters.

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod ports;
pub mod rules;
