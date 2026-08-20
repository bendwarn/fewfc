//! CFECards 的確定性規則引擎。
//!
//! 此 crate 依照整潔架構邊界分為：領域模型、應用程式編排、規則註冊表、
//! 連接埠，以及基礎設施轉接器。

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod ports;
pub mod public_view;
pub mod rules;
pub mod web_api;

#[cfg(target_arch = "wasm32")]
pub mod wasm;
