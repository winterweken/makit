//! Geometry primitives and backend-agnostic braille drawing for makit.
//!
//! This crate provides the core 2D geometry types (Point, Line, Rectangle),
//! drawing utilities that render onto any [`Plotter`] (a dependency-free
//! [`BrailleGrid`] for plain-text output, or a [`Recording`] that the TUI
//! replays onto a ratatui canvas), and SDF (Signed Distance Field) primitives
//! for smooth shape rendering.

pub mod braille;
pub mod drawing;
pub mod plot;
pub mod sdf;
pub mod types;

pub use braille::BrailleGrid;
pub use drawing::*;
pub use plot::{Plotter, Recording};
pub use types::*;
