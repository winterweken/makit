//! HTTP client for the pyRevit extension API.
//!
//! The pyRevit extension (in `pyrevit-extension/`) runs inside Revit and
//! exposes an HTTP API on port 48884. This client talks to it using reqwest.

use super::models::*;
use anyhow::{Context, Result};

/// Default pyRevit HTTP bridge port.
const DEFAULT_PORT: u16 = 48884;

/// Build the base URL for the pyRevit server.
fn base_url(port: u16) -> String {
    format!("http://localhost:{}", port)
}

/// Check whether the pyRevit server is reachable.
pub async fn check_connection(port: u16) -> Result<bool> {
    let url = format!("{}/api/status", base_url(port));
    match reqwest::get(&url).await {
        Ok(resp) => Ok(resp.status().is_success()),
        Err(_) => Ok(false),
    }
}

/// Extract wall elements from the running Revit model.
pub async fn extract_walls(port: u16) -> Result<Vec<WallData>> {
    let url = format!("{}/api/walls", base_url(port));
    let resp = reqwest::get(&url)
        .await
        .context(connection_error_message())?;

    if !resp.status().is_success() {
        anyhow::bail!(
            "Revit API returned status {}: {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        );
    }

    let walls: Vec<WallData> = resp
        .json()
        .await
        .context("Failed to parse wall data from Revit API")?;

    Ok(walls)
}

/// Extract room elements from the running Revit model.
pub async fn extract_rooms(port: u16) -> Result<Vec<RoomData>> {
    let url = format!("{}/api/rooms", base_url(port));
    let resp = reqwest::get(&url)
        .await
        .context(connection_error_message())?;

    if !resp.status().is_success() {
        anyhow::bail!(
            "Revit API returned status {}: {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        );
    }

    let rooms: Vec<RoomData> = resp
        .json()
        .await
        .context("Failed to parse room data from Revit API")?;

    Ok(rooms)
}

/// Extract floor elements from the running Revit model.
pub async fn extract_floors(port: u16) -> Result<Vec<FloorData>> {
    let url = format!("{}/api/floors", base_url(port));
    let resp = reqwest::get(&url)
        .await
        .context(connection_error_message())?;

    if !resp.status().is_success() {
        anyhow::bail!(
            "Revit API returned status {}: {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        );
    }

    let floors: Vec<FloorData> = resp
        .json()
        .await
        .context("Failed to parse floor data from Revit API")?;

    Ok(floors)
}

/// Standard connection error message.
fn connection_error_message() -> String {
    format!(
        "Revit not connected — ensure the pyRevit extension is loaded \
         and Revit is running on port {}",
        DEFAULT_PORT
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_url() {
        assert_eq!(base_url(48884), "http://localhost:48884");
        assert_eq!(base_url(9090), "http://localhost:9090");
    }

    // -----------------------------------------------------------------------
    // Mock pyRevit server — serves realistic JSON on /api/* endpoints
    // -----------------------------------------------------------------------

    use axum::{http::StatusCode, routing::get, Json, Router};

    fn mock_walls() -> Vec<WallData> {
        vec![
            WallData {
                id: 100,
                wall_type: "Generic - 200mm".into(),
                level: "Level 1".into(),
                area_sqm: 30.0,
                length_m: 10.0,
                height_m: 3.0,
                start_point: [0.0, 0.0, 0.0],
                end_point: [10.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
            },
            WallData {
                id: 101,
                wall_type: "Curtain Wall".into(),
                level: "Level 2".into(),
                area_sqm: 45.5,
                length_m: 12.0,
                height_m: 3.5,
                start_point: [0.0, 0.0, 3.0],
                end_point: [0.0, 12.0, 3.0],
                normal: [-1.0, 0.0, 0.0],
            },
        ]
    }

    fn mock_rooms() -> Vec<RoomData> {
        vec![RoomData {
            id: 200,
            name: "Office".into(),
            number: "101".into(),
            level: "Level 1".into(),
            area_sqm: 25.0,
            perimeter_m: 20.0,
            boundary_points: vec![[0.0, 0.0], [5.0, 0.0], [5.0, 5.0], [0.0, 5.0]],
        }]
    }

    fn mock_floors() -> Vec<FloorData> {
        vec![FloorData {
            id: 300,
            floor_type: "Concrete 150mm".into(),
            level: "Level 1".into(),
            area_sqm: 120.0,
            thickness_m: 0.15,
        }]
    }

    /// Build a mock pyRevit HTTP server router.
    fn mock_revit_router() -> Router {
        Router::new()
            .route("/api/status", get(|| async { StatusCode::OK }))
            .route("/api/walls", get(|| async { Json(mock_walls()) }))
            .route("/api/rooms", get(|| async { Json(mock_rooms()) }))
            .route("/api/floors", get(|| async { Json(mock_floors()) }))
    }

    /// Build a mock server that returns 500 on all API endpoints.
    fn mock_error_router() -> Router {
        Router::new()
            .route(
                "/api/status",
                get(|| async { StatusCode::INTERNAL_SERVER_ERROR }),
            )
            .route(
                "/api/walls",
                get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "server error") }),
            )
            .route(
                "/api/rooms",
                get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "server error") }),
            )
            .route(
                "/api/floors",
                get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "server error") }),
            )
    }

    /// Start a mock server on an OS-assigned port and return the port.
    async fn start_mock(router: Router) -> u16 {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        port
    }

    // -----------------------------------------------------------------------
    // Integration tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn integration_check_connection_reachable() {
        let port = start_mock(mock_revit_router()).await;
        let connected = check_connection(port).await.unwrap();
        assert!(connected, "mock server should be reachable");
    }

    #[tokio::test]
    async fn integration_check_connection_unreachable() {
        // Port 1 is almost certainly not listening
        let connected = check_connection(1).await.unwrap();
        assert!(!connected, "nothing should be listening on port 1");
    }

    #[tokio::test]
    async fn integration_check_connection_returns_false_on_500() {
        let port = start_mock(mock_error_router()).await;
        let connected = check_connection(port).await.unwrap();
        assert!(!connected, "500 status should report as not connected");
    }

    #[tokio::test]
    async fn integration_extract_walls() {
        let port = start_mock(mock_revit_router()).await;
        let walls = extract_walls(port).await.unwrap();

        assert_eq!(walls.len(), 2);
        assert_eq!(walls[0].id, 100);
        assert_eq!(walls[0].wall_type, "Generic - 200mm");
        assert!((walls[0].area_sqm - 30.0).abs() < 0.001);

        assert_eq!(walls[1].id, 101);
        assert_eq!(walls[1].wall_type, "Curtain Wall");
        assert!((walls[1].area_sqm - 45.5).abs() < 0.001);
    }

    #[tokio::test]
    async fn integration_extract_rooms() {
        let port = start_mock(mock_revit_router()).await;
        let rooms = extract_rooms(port).await.unwrap();

        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].name, "Office");
        assert_eq!(rooms[0].number, "101");
        assert_eq!(rooms[0].boundary_points.len(), 4);
    }

    #[tokio::test]
    async fn integration_extract_floors() {
        let port = start_mock(mock_revit_router()).await;
        let floors = extract_floors(port).await.unwrap();

        assert_eq!(floors.len(), 1);
        assert_eq!(floors[0].floor_type, "Concrete 150mm");
        assert!((floors[0].area_sqm - 120.0).abs() < 0.001);
        assert!((floors[0].thickness_m - 0.15).abs() < 0.001);
    }

    #[tokio::test]
    async fn integration_extract_walls_server_error() {
        let port = start_mock(mock_error_router()).await;
        let result = extract_walls(port).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("500"), "error should mention status code");
    }

    #[tokio::test]
    async fn integration_extract_walls_connection_refused() {
        // Port 1 — nothing listening
        let result = extract_walls(1).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Revit not connected"),
            "error should mention Revit: {}",
            err_msg
        );
    }
}
