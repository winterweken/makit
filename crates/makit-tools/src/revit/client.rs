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
}
