//! Rhino source + import/export/Grasshopper actions.

use makit_core::registry::Registry;
use std::sync::Arc;

pub fn register_tasks(reg: &mut Registry) {
    reg.register_source(
        "rhino",
        "Rhino 3D / Grasshopper integration",
        Arc::new(|_ctx| {
            println!("Connecting to Rhino...");
            Ok(())
        }),
    );

    reg.register_action(
        "rhino-import-revit",
        "Import Revit model into Rhino",
        "import-export",
        Arc::new(|_ctx| {
            println!("Importing Revit model...");
            Ok(())
        }),
    )
    .add_option("input", "Input file path", "string", true, None);

    reg.register_action(
        "rhino-run-definition",
        "Run a Grasshopper definition",
        "grasshopper",
        Arc::new(|_ctx| {
            println!("Running Grasshopper definition...");
            Ok(())
        }),
    )
    .add_option("definition", "Path to .gh file", "string", true, None);
}
