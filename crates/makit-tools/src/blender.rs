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

    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

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
    }

    // -----------------------------------------------------------------------
    // Integration tests — exercise the full router via tower::oneshot
    // -----------------------------------------------------------------------

    /// Helper: build a fresh router + shared state for each test.
    fn test_app() -> (Router, SyncState) {
        let state = SyncState::default();
        let router = build_router(state.clone());
        (router, state)
    }

    /// Helper: collect an axum response body into bytes.
    async fn body_bytes(body: Body) -> Vec<u8> {
        body.collect().await.unwrap().to_bytes().to_vec()
    }

    #[tokio::test]
    async fn integration_health_returns_ok() {
        let (app, _) = test_app();

        let req = axum::http::Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let text = String::from_utf8(body_bytes(resp.into_body()).await).unwrap();
        assert!(text.contains("makit blender sync server"));
    }

    #[tokio::test]
    async fn integration_get_geometry_empty_returns_404() {
        let (app, _) = test_app();

        let req = axum::http::Request::builder()
            .uri("/geometry")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn integration_post_then_get_geometry() {
        let state = SyncState::default();

        // POST a mesh
        let mesh = MeshData {
            vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            faces: vec![vec![0, 1, 2]],
            name: "Triangle".to_string(),
        };
        let payload = serde_json::to_string(&mesh).unwrap();

        let post_req = axum::http::Request::builder()
            .method("POST")
            .uri("/geometry")
            .header("content-type", "application/json")
            .body(Body::from(payload))
            .unwrap();

        let app = build_router(state.clone());
        let resp = app.oneshot(post_req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = String::from_utf8(body_bytes(resp.into_body()).await).unwrap();
        assert!(body.contains("3 vertices"));
        assert!(body.contains("1 faces"));
        assert!(body.contains("Triangle"));

        // GET the stored geometry back
        let get_req = axum::http::Request::builder()
            .uri("/geometry")
            .body(Body::empty())
            .unwrap();

        let app = build_router(state.clone());
        let resp = app.oneshot(get_req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let returned: MeshData =
            serde_json::from_slice(&body_bytes(resp.into_body()).await).unwrap();
        assert_eq!(returned.name, "Triangle");
        assert_eq!(returned.vertices.len(), 3);
        assert_eq!(returned.faces.len(), 1);
    }

    #[tokio::test]
    async fn integration_post_replaces_previous_mesh() {
        let state = SyncState::default();

        // POST first mesh
        let mesh1 = MeshData {
            vertices: vec![[0.0, 0.0, 0.0]],
            faces: vec![],
            name: "First".to_string(),
        };

        let app = build_router(state.clone());
        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/geometry")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&mesh1).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // POST second mesh
        let mesh2 = MeshData {
            vertices: vec![[1.0, 1.0, 1.0], [2.0, 2.0, 2.0]],
            faces: vec![vec![0, 1]],
            name: "Second".to_string(),
        };

        let app = build_router(state.clone());
        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/geometry")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&mesh2).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // GET should return the second mesh, not the first
        let app = build_router(state.clone());
        let req = axum::http::Request::builder()
            .uri("/geometry")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let returned: MeshData =
            serde_json::from_slice(&body_bytes(resp.into_body()).await).unwrap();
        assert_eq!(returned.name, "Second");
        assert_eq!(returned.vertices.len(), 2);
    }

    #[tokio::test]
    async fn integration_post_invalid_json_returns_error() {
        let (app, _) = test_app();

        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/geometry")
            .header("content-type", "application/json")
            .body(Body::from("{not valid json}"))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        // axum returns 422 Unprocessable Entity for JSON parse failures
        assert!(resp.status().is_client_error());
    }

    #[tokio::test]
    async fn integration_mesh_without_name_defaults_to_empty() {
        let state = SyncState::default();

        let json = r#"{"vertices": [[1.0, 2.0, 3.0]], "faces": []}"#;

        let app = build_router(state.clone());
        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/geometry")
            .header("content-type", "application/json")
            .body(Body::from(json))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Verify the default empty name round-trips
        let stored = state.mesh.read().unwrap();
        let mesh = stored.as_ref().unwrap();
        assert!(mesh.name.is_empty());
        assert_eq!(mesh.vertices.len(), 1);
    }
}
