//! Core registry, config, and types for makit.
//!
//! The registry provides a plugin-based architecture where tools register
//! sources (geometry input drivers) and actions (operations on geometry).

pub mod config;
pub mod models;
pub mod registry;

pub use config::Config;
pub use models::*;
pub use registry::Registry;
