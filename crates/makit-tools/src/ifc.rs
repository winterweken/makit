//! IFC file source.

use makit_core::registry::Registry;
use std::sync::Arc;

pub fn register_tasks(reg: &mut Registry) {
    reg.register_source(
        "ifc",
        "IFC file loader",
        Arc::new(|_ctx| {
            println!("Loading IFC file...");
            Ok(())
        }),
    )
    .add_option("file", "Path to IFC file", "string", true, None);
}
