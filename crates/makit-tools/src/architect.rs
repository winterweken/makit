//! Architectural rendering demo action.

use std::sync::Arc;
use makit_core::registry::Registry;

pub fn register_tasks(reg: &mut Registry) {
    reg.register_action("architect-render-demo", "Render architectural demo", "rendering", Arc::new(|_ctx| {
        println!("Rendering architectural demo...");
        Ok(())
    }));
}
