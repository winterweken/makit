//! Revit data models for the pyRevit HTTP bridge.
//!
//! These structs mirror the JSON responses from the pyRevit extension's
//! HTTP API running inside Revit.

use serde::{Deserialize, Serialize};

/// Data for a single wall element extracted from Revit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallData {
    pub id: i64,
    #[serde(default)]
    pub wall_type: String,
    #[serde(default)]
    pub level: String,
    pub area_sqm: f64,
    #[serde(default)]
    pub length_m: f64,
    #[serde(default)]
    pub height_m: f64,
    /// Start point [x, y, z] in Revit internal coordinates.
    #[serde(default)]
    pub start_point: [f64; 3],
    /// End point [x, y, z] in Revit internal coordinates.
    #[serde(default)]
    pub end_point: [f64; 3],
    /// Outward-facing normal vector [nx, ny, nz].
    #[serde(default)]
    pub normal: [f64; 3],
}

/// Data for a single room element extracted from Revit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomData {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub number: String,
    #[serde(default)]
    pub level: String,
    pub area_sqm: f64,
    #[serde(default)]
    pub perimeter_m: f64,
    /// Room boundary as a list of 2D points [[x, y], ...]
    #[serde(default)]
    pub boundary_points: Vec<[f64; 2]>,
}

/// Data for a single floor element extracted from Revit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FloorData {
    pub id: i64,
    #[serde(default)]
    pub floor_type: String,
    #[serde(default)]
    pub level: String,
    pub area_sqm: f64,
    #[serde(default)]
    pub thickness_m: f64,
}

/// Wall orientation analysis result for a single cardinal direction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrientationResult {
    pub direction: String,
    pub count: usize,
    pub total_area_sqm: f64,
    pub percentage: f64,
}

/// Compute cardinal direction from a normal vector's XY components.
pub fn cardinal_direction(nx: f64, ny: f64) -> &'static str {
    let angle = ny.atan2(nx).to_degrees();
    // Normalize to 0-360
    let angle = if angle < 0.0 { angle + 360.0 } else { angle };
    match angle as i32 {
        315..=360 | 0..=44 => "East",
        45..=134 => "North",
        135..=224 => "West",
        225..=314 => "South",
        _ => "Unknown",
    }
}

/// Analyze wall orientations and compute area breakdown by cardinal direction.
pub fn analyze_orientations(walls: &[WallData]) -> Vec<OrientationResult> {
    let mut buckets: std::collections::HashMap<&str, (usize, f64)> = std::collections::HashMap::new();

    for wall in walls {
        let dir = cardinal_direction(wall.normal[0], wall.normal[1]);
        let entry = buckets.entry(dir).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += wall.area_sqm;
    }

    let total_area: f64 = walls.iter().map(|w| w.area_sqm).sum();

    let mut results: Vec<OrientationResult> = buckets
        .into_iter()
        .map(|(dir, (count, area))| OrientationResult {
            direction: dir.to_string(),
            count,
            total_area_sqm: (area * 100.0).round() / 100.0,
            percentage: if total_area > 0.0 {
                (area / total_area * 100.0 * 10.0).round() / 10.0
            } else {
                0.0
            },
        })
        .collect();

    results.sort_by(|a, b| {
        let order = |d: &str| match d {
            "North" => 0, "East" => 1, "South" => 2, "West" => 3, _ => 4
        };
        order(&a.direction).cmp(&order(&b.direction))
    });

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cardinal_direction() {
        assert_eq!(cardinal_direction(0.0, 1.0), "North");
        assert_eq!(cardinal_direction(1.0, 0.0), "East");
        assert_eq!(cardinal_direction(0.0, -1.0), "South");
        assert_eq!(cardinal_direction(-1.0, 0.0), "West");
    }

    #[test]
    fn test_analyze_orientations() {
        let walls = vec![
            WallData {
                id: 1, wall_type: "Basic".into(), level: "L1".into(),
                area_sqm: 50.0, length_m: 10.0, height_m: 3.0,
                start_point: [0.0, 0.0, 0.0], end_point: [10.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
            },
            WallData {
                id: 2, wall_type: "Basic".into(), level: "L1".into(),
                area_sqm: 30.0, length_m: 8.0, height_m: 3.0,
                start_point: [0.0, 0.0, 0.0], end_point: [0.0, 8.0, 0.0],
                normal: [0.0, -1.0, 0.0],
            },
        ];

        let results = analyze_orientations(&walls);
        assert_eq!(results.len(), 2);
        let north = results.iter().find(|r| r.direction == "North").unwrap();
        assert_eq!(north.count, 1);
        assert!((north.total_area_sqm - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_wall_data_deserialization() {
        let json = r#"{
            "id": 42,
            "wall_type": "Generic - 200mm",
            "level": "Level 1",
            "area_sqm": 24.5,
            "length_m": 5.0,
            "height_m": 3.0,
            "start_point": [0.0, 0.0, 0.0],
            "end_point": [5.0, 0.0, 0.0],
            "normal": [0.0, 1.0, 0.0]
        }"#;

        let wall: WallData = serde_json::from_str(json).unwrap();
        assert_eq!(wall.id, 42);
        assert_eq!(wall.wall_type, "Generic - 200mm");
        assert!((wall.area_sqm - 24.5).abs() < 0.001);
    }
}
