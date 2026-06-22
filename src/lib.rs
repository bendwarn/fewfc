//! Deterministic rules engine for CFECards.
//!
//! The crate is split by Clean Architecture boundaries: domain model,
//! application orchestration, rule registries, ports, and infrastructure
//! adapters.

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod ports;
pub mod public_view;
pub mod rules;
