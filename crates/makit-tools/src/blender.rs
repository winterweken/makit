//! Blender live geometry sync server.
//!
//! Starts an axum HTTP server that receives mesh geometry from Blender
//! via the companion addon in `scripts/blender/`.

use std::sync::{Arc, RwLock};

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use makit_core::models::TaskContext;
use makit_core::registry::Registry;

// ---------------------------------------------------------------------------
// Data Types
// ---------------------------------------------------------------------------

/// Mesh geometry payload sent from Blender.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshData {
    /// Vertex positions as [[x, y, z], ...]
    pub vertices: Vec<[f64; 3]>,
    /// Face indices as [[v0, v1, v2], ...] or [[v0, v1, v2, v3], ...]
    pub faces: Vec<Vec<usize>>,
    /// Optional mesh name from Blender.
    #[serde(default)]
    pub name: String,
}

/// Shared application state for the sync server.
#[derive(Debug, Clone, Default)]
pub struct SyncState {
    /// Most recently received geometry.
    pub mesh: Arc<RwLock<Option<MeshData>>>,
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// Build the axum router for the Blender sync server.
fn build_router(state: SyncState) -> Router {
    Router::new()
        .route("/health", get(handle_health))
        .route("/geometry", post(handle_geometry))
        .route("/geometry", get(handle_get_geometry))
        .with_state(state)
}

/// Health check endpoint.
async fn handle_health() -> impl IntoResponse {
    (StatusCode::OK, "makit blender sync server")
}

/// Receive geometry from Blender (POST /geometry).
async fn handle_geometry(
    State(state): State<SyncState>,
    Json(mesh): Json<MeshData>,
) -> impl IntoResponse {
    let n_verts = mesh.vertices.len();
    let n_faces = mesh.faces.len();
    let name = mesh.name.clone();

    // Store in shared state
    if let Ok(mut lock) = state.mesh.write() {
        *lock = Some(mesh);
    }

    let msg = format!(
        "Received mesh: {} vertices, {} faces{}",
        n_verts,
        n_faces,
        if name.is_empty() {
            String::new()
        } else {
            format!(" ({})", name)
        }
    );
    println!("  {}", msg);
    (StatusCode::OK, msg)
}

/// Retrieve the current geometry (GET /geometry).
async fn handle_get_geometry(State(state): State<SyncState>) -> impl IntoResponse {
    let mesh = state.mesh.read().ok().and_then(|lock| lock.clone());
    match mesh {
        Some(m) => (StatusCode::OK, Json(Some(m))),
        None => (StatusCode::NOT_FOUND, Json(None)),
    }
}

/// Start the Blender sync server on the given port.
pub async fn start_server(port: u16) -> anyhow::Result<()> {
    let state = SyncState::default();
    let app = build_router(state);

    let addr = format!("0.0.0.0:{}", port);
    println!("Starting Blender sync server on http://localhost:{}", port);
    println!("  POST /geometry — send mesh data from Blender");
    println!("  GET  /geometry — retrieve current mesh");
    println!("  GET  /health   — check server status");
    println!();
    println!("Press Ctrl+C to stop.");

    let listener = tokio::net::TcpListener::bind(&addr).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::AddrInUse {
            anyhow::anyhow!(
                "Port {} is occupied — stop the existing server or choose \
                     another port with --port <N>",
                port
            )
        } else {
            anyhow::anyhow!("Failed to bind to {}: {}", addr, e)
        }
    })?;

    axum::serve(listener, app).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub fn register_tasks(reg: &mut Registry) {
    reg.register_source(
        "blender",
        "Blender live geometry sync",
        Arc::new(handle_blender),
    )
    .add_option("port", "Server port", "int", false, Some("8085"));
}

/// Handle `blender` source — starts the sync server.
fn handle_blender(ctx: &TaskContext) -> anyhow::Result<()> {
    let port: u16 = ctx.get_option("port", "8085").parse()?;

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(start_server(port))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_data_deserialization() {
        let json = r#"{
            "vertices": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
            "faces": [[0, 1, 2]],
            "name": "Cube"
        }"#;

        let mesh: MeshData = serde_json::from_str(json).unwrap();
        assert_eq!(mesh.vertices.len(), 3);
        assert_eq!(mesh.faces.len(), 1);
        assert_eq!(mesh.name, "Cube");
    }

    #[test]
    fn test_sync_state_default() {
        let state = SyncState::default();
        let mesh = state.mesh.read().unwrap();
        assert!(mesh.is_none());
    }

    #[test]
    fn test_router_builds() {
        let state = SyncState::default();
        let _router = build_router(state);
        // If this compiles and runs, the router is valid
    }
}
