//! Core registry, config, and types for makit.
//!
//! The registry provides a plugin-based architecture where tools register
//! sources (geometry input drivers) and actions (operations on geometry).

pub mod registry;
pub mod models;
pub mod config;

pub use registry::Registry;
pub use models::*;
pub use config::Config;
