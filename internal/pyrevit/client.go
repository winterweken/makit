package pyrevit

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"
)

const (
	defaultEndpoint = "http://localhost:8888"
	defaultTimeout  = 30 * time.Second
)

// Client handles communication with the pyRevit HTTP server
type Client struct {
	endpoint   string
	httpClient *http.Client
}

// NewClient creates a new pyRevit client
func NewClient(endpoint string) *Client {
	if endpoint == "" {
		endpoint = defaultEndpoint
	}

	return &Client{
		endpoint: endpoint,
		httpClient: &http.Client{
			Timeout: defaultTimeout,
		},
	}
}

// HealthCheck verifies the pyRevit server is running
func (c *Client) HealthCheck() error {
	resp, err := c.httpClient.Get(c.endpoint + "/health")
	if err != nil {
		return fmt.Errorf("pyRevit server not reachable: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("pyRevit server unhealthy: status %d", resp.StatusCode)
	}

	return nil
}

// Get performs a GET request to the pyRevit server
func (c *Client) Get(path string, result interface{}) error {
	resp, err := c.httpClient.Get(c.endpoint + path)
	if err != nil {
		return fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("request failed with status %d: %s", resp.StatusCode, string(body))
	}

	if err := json.NewDecoder(resp.Body).Decode(result); err != nil {
		return fmt.Errorf("failed to decode response: %w", err)
	}

	return nil
}

// Post performs a POST request to the pyRevit server
func (c *Client) Post(path string, payload interface{}, result interface{}) error {
	data, err := json.Marshal(payload)
	if err != nil {
		return fmt.Errorf("failed to marshal payload: %w", err)
	}

	resp, err := c.httpClient.Post(
		c.endpoint+path,
		"application/json",
		bytes.NewBuffer(data),
	)
	if err != nil {
		return fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("request failed with status %d: %s", resp.StatusCode, string(body))
	}

	if result != nil {
		if err := json.NewDecoder(resp.Body).Decode(result); err != nil {
			return fmt.Errorf("failed to decode response: %w", err)
		}
	}

	return nil
}

// ExtractWalls extracts wall geometry from the active Revit model
func (c *Client) ExtractWalls(options WallExtractionOptions) (*GeometryResponse, error) {
	var result GeometryResponse
	err := c.Post("/api/geometry/walls", options, &result)
	if err != nil {
		return nil, err
	}
	return &result, nil
}

// ExtractFloors extracts floor geometry from the active Revit model
func (c *Client) ExtractFloors(options FloorExtractionOptions) (*GeometryResponse, error) {
	var result GeometryResponse
	err := c.Post("/api/geometry/floors", options, &result)
	if err != nil {
		return nil, err
	}
	return &result, nil
}

// ExtractRooms extracts room data from the active Revit model
func (c *Client) ExtractRooms(options RoomExtractionOptions) (*RoomsResponse, error) {
	var result RoomsResponse
	err := c.Post("/api/geometry/rooms", options, &result)
	if err != nil {
		return nil, err
	}
	return &result, nil
}

// GetProjectInfo retrieves information about the active Revit project
func (c *Client) GetProjectInfo() (*ProjectInfo, error) {
	var result ProjectInfo
	err := c.Get("/api/project/info", &result)
	if err != nil {
		return nil, err
	}
	return &result, nil
}

// AnalyzeWallOrientations analyzes wall orientations and calculates WWR
// This is a hybrid approach that extracts and analyzes in one step
func (c *Client) AnalyzeWallOrientations(options WallOrientationOptions) (*WallOrientationResponse, error) {
	var result WallOrientationResponse
	err := c.Post("/api/analysis/wall-orientations", options, &result)
	if err != nil {
		return nil, err
	}
	return &result, nil
}

// ExtractBuildingModel extracts a complete building model in generic format
// This allows for caching and reuse across multiple analysis operations
func (c *Client) ExtractBuildingModel(options BuildingModelExtractionOptions) (map[string]interface{}, error) {
	var result map[string]interface{}
	err := c.Post("/api/extraction/building-model", options, &result)
	if err != nil {
		return nil, err
	}
	return result, nil
}

// AnalyzeGenericModel analyzes a pre-extracted building model
// This is the pure generic approach - model can come from any source
func (c *Client) AnalyzeGenericModel(buildingModel map[string]interface{}) (*WallOrientationResponse, error) {
	payload := map[string]interface{}{
		"buildingModel": buildingModel,
	}

	var result WallOrientationResponse
	err := c.Post("/api/analysis/generic", payload, &result)
	if err != nil {
		return nil, err
	}
	return &result, nil
}
