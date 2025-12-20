package blender

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
)

type Server struct {
	port int
}

func NewServer(port int) *Server {
	return &Server{port: port}
}

func (s *Server) Start() error {
	mux := http.NewServeMux()
	mux.HandleFunc("/geometry", s.handleGeometry)

	addr := fmt.Sprintf(":%d", s.port)

	// This blocks, so it works well as a "Task" execution in the TUI context
	return http.ListenAndServe(addr, mux)
}

func (s *Server) handleGeometry(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	body, err := io.ReadAll(r.Body)
	if err != nil {
		http.Error(w, "Failed to read body", http.StatusInternalServerError)
		return
	}
	defer r.Body.Close()

	// Parse generically first to understand structure
	var payload struct {
		Source string                 `json:"source"`
		Data   map[string]interface{} `json:"data"`
	}

	if err := json.Unmarshal(body, &payload); err != nil {
		// fmt.Printf("Error unmarshaling payload: %v\n", err)
		http.Error(w, "Invalid JSON", http.StatusBadRequest)
		return
	}

	// fmt.Printf("[%s] Received geometry update from %s\n", time.Now().Format(time.TimeOnly), payload.Source)

	// Transform Blender Standard to Makit Viz Standard
	// Our Blender script sends {geometry: [...]} which is what we want?
	// Actually, the TUI expects specific formats.
	//
	// The current TUI logic (loadIsometricFaces) expects:
	// map[string] { "faces": [...], "stats": {...} } keyed by direction name.
	//
	// Our Blender script produces "geometry": [...] list of faces.
	// We should wrap this in an "Overview" direction for the TUI to pick it up.

	vizData := make(map[string]interface{})

	// Get geometry array
	geoList, ok := payload.Data["geometry"].([]interface{})
	if !ok {
		fmt.Println("No geometry found in payload")
		return
	}

	// Create Overview entry
	overview := map[string]interface{}{
		"faces": geoList,
		"stats": map[string]interface{}{
			"walls":   float64(len(geoList)), // simplified
			"windows": 0.0,
			"wwr":     0.0,
		},
	}

	vizData["Overview"] = overview

	// Save to /tmp/makit_viz.json
	vizFile := filepath.Join(os.TempDir(), "makit_viz.json")
	if err := saveJSON(vizData, vizFile); err != nil {
		// fmt.Printf("Error saving viz data: %v\n", err)
		http.Error(w, "Failed to save data", http.StatusInternalServerError)
		return
	}

	// Also save to /tmp/makit_viz.json for fallback (legacy support)
	saveJSON(vizData, "/tmp/makit_viz.json")

	w.WriteHeader(http.StatusOK)
	w.Write([]byte("Geometry received"))
}

func saveJSON(data interface{}, path string) error {
	bytes, err := json.MarshalIndent(data, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(path, bytes, 0644)
}
