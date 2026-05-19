//! Blender live geometry sync server.

use std::sync::Arc;
use makit_core::registry::Registry;

pub fn register_tasks(reg: &mut Registry) {
    reg.register_source("blender", "Blender live geometry sync", Arc::new(|_ctx| {
        println!("Starting Blender sync server on port 8085...");
        Ok(())
    }));
}
