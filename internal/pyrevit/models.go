package pyrevit

// Point3D represents a 3D point
type Point3D struct {
	X float64 `json:"x"`
	Y float64 `json:"y"`
	Z float64 `json:"z"`
}

// Line3D represents a 3D line segment
type Line3D struct {
	Start Point3D `json:"start"`
	End   Point3D `json:"end"`
}

// BoundingBox represents a 3D bounding box
type BoundingBox struct {
	Min Point3D `json:"min"`
	Max Point3D `json:"max"`
}

// ElementGeometry represents geometry data for a Revit element
type ElementGeometry struct {
	ID         int         `json:"id"`
	Category   string      `json:"category"`
	Name       string      `json:"name"`
	Level      string      `json:"level"`
	Lines      []Line3D    `json:"lines"`
	BoundingBox BoundingBox `json:"boundingBox"`
	Parameters map[string]interface{} `json:"parameters"`
}

// GeometryResponse is the response from geometry extraction endpoints
type GeometryResponse struct {
	Elements []ElementGeometry `json:"elements"`
	Count    int               `json:"count"`
	Message  string            `json:"message"`
}

// Room represents a Revit room with geometry and properties
type Room struct {
	ID         int                    `json:"id"`
	Name       string                 `json:"name"`
	Number     string                 `json:"number"`
	Level      string                 `json:"level"`
	Area       float64                `json:"area"`
	Perimeter  float64                `json:"perimeter"`
	Height     float64                `json:"height"`
	Boundaries []Line3D               `json:"boundaries"`
	Parameters map[string]interface{} `json:"parameters"`
}

// RoomsResponse is the response from room extraction
type RoomsResponse struct {
	Rooms   []Room `json:"rooms"`
	Count   int    `json:"count"`
	Message string `json:"message"`
}

// ProjectInfo contains information about the Revit project
type ProjectInfo struct {
	Name          string `json:"name"`
	Number        string `json:"number"`
	Address       string `json:"address"`
	Author        string `json:"author"`
	ClientName    string `json:"clientName"`
	RevitVersion  string `json:"revitVersion"`
	IsWorkshared  bool   `json:"isWorkshared"`
	CentralPath   string `json:"centralPath"`
}

// WallExtractionOptions configures wall extraction
type WallExtractionOptions struct {
	Level          string   `json:"level,omitempty"`
	WallTypes      []string `json:"wallTypes,omitempty"`
	IncludeCurved  bool     `json:"includeCurved"`
	IncludeStacked bool     `json:"includeStacked"`
}

// FloorExtractionOptions configures floor extraction
type FloorExtractionOptions struct {
	Level          string   `json:"level,omitempty"`
	FloorTypes     []string `json:"floorTypes,omitempty"`
	AreaThreshold  float64  `json:"areaThreshold,omitempty"`
}

// RoomExtractionOptions configures room extraction
type RoomExtractionOptions struct {
	Level           string  `json:"level,omitempty"`
	IncludeUnplaced bool    `json:"includeUnplaced"`
	AreaThreshold   float64 `json:"areaThreshold,omitempty"`
}

// AnalysisRequest represents a request for analysis
type AnalysisRequest struct {
	AnalysisType string                 `json:"analysisType"`
	Options      map[string]interface{} `json:"options"`
}

// AnalysisResponse contains analysis results
type AnalysisResponse struct {
	Results map[string]interface{} `json:"results"`
	Message string                 `json:"message"`
}

// WallOrientationOptions configures wall orientation analysis
type WallOrientationOptions struct {
	Workset      string `json:"workset,omitempty"`
	WallType     string `json:"wallType,omitempty"`
	AreaUnit     string `json:"areaUnit,omitempty"`
	IncludeWindows bool `json:"includeWindows"`
}

// DirectionStats contains statistics for a specific compass direction
type DirectionStats struct {
	WallCount   int      `json:"wallCount"`
	WallArea    float64  `json:"wallArea"`
	WindowCount int      `json:"windowCount"`
	WindowArea  float64  `json:"windowArea"`
	WWR         float64  `json:"wwr"`
	WallIDs     []string `json:"wallIds"`
	WindowIDs   []string `json:"windowIds"`
}

// TotalStats contains overall statistics
type TotalStats struct {
	WallCount   int     `json:"wallCount"`
	WallArea    float64 `json:"wallArea"`
	WindowCount int     `json:"windowCount"`
	WindowArea  float64 `json:"windowArea"`
	WWR         float64 `json:"wwr"`
}

// WallOrientationStats contains complete orientation analysis results
type WallOrientationStats struct {
	ByDirection       map[string]DirectionStats `json:"byDirection"`
	Totals            TotalStats                `json:"totals"`
	ProjectNorth      float64                   `json:"projectNorth"`
	Units             string                    `json:"units"`
	WallOrientations  map[string]string         `json:"wallOrientations"`
}

// WallOrientationResponse is the response from wall orientation analysis
type WallOrientationResponse struct {
	Stats         WallOrientationStats   `json:"stats"`
	Report        string                 `json:"report"`
	BuildingModel map[string]interface{} `json:"buildingModel,omitempty"`
}

// BuildingModelExtractionOptions configures building model extraction
type BuildingModelExtractionOptions struct {
	Workset        string `json:"workset,omitempty"`
	WallType       string `json:"wallType,omitempty"`
	AreaUnit       string `json:"areaUnit,omitempty"`
	IncludeWindows bool   `json:"includeWindows"`
}
