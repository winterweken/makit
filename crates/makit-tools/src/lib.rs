//! Tool implementations for makit.
//!
//! Each module registers sources and/or actions with the global registry.

pub mod analysis;
pub mod architect;
pub mod blender;
pub mod building_model;
pub mod ifc;
pub mod murb;
pub mod revit;
pub mod rhino;

/// Register all available tools with the global registry.
pub fn register_all_tools() {
    let reg = makit_core::Registry::global();
    let mut reg = reg.write().unwrap();

    revit::register_tasks(&mut reg);
    rhino::register_tasks(&mut reg);
    blender::register_tasks(&mut reg);
    ifc::register_tasks(&mut reg);
    analysis::register_tasks(&mut reg);
    architect::register_tasks(&mut reg);
    murb::register_tasks(&mut reg);
}
