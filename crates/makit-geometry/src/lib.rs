//! Geometry primitives and braille canvas drawing for makit.
//!
//! This crate provides the core 2D geometry types (Point, Line, Rectangle)
//! and drawing utilities that use rsille's braille canvas for terminal rendering.

pub mod types;
pub mod drawing;

pub use types::*;
pub use drawing::*;
