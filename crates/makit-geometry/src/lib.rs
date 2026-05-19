//! Geometry primitives and braille canvas drawing for makit.
//!
//! This crate provides the core 2D geometry types (Point, Line, Rectangle),
//! drawing utilities that use rsille's braille canvas for terminal rendering,
//! and SDF (Signed Distance Field) primitives for smooth shape rendering.

pub mod drawing;
pub mod sdf;
pub mod types;

pub use drawing::*;
pub use types::*;
