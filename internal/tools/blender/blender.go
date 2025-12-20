package blender

import (
	"fmt"

	"github.com/winteweken/makit/internal/registry"
)

var globalServer *Server

// RegisterTasks registers all Blender-related tasks
func RegisterTasks() {
	reg := registry.GetRegistry()

	// Register as a Source
	reg.RegisterSource("blender", "Blender 3D live connection", handleStartServer).
		AddOption("port", "Port to listen on", "int", false, 8085)
}

// Deprecated: registerSyncTasks is removed as we registered the source directly

func handleStartServer(ctx *registry.TaskContext) error {
	port := 8085
	if p, ok := ctx.Options["port"].(int); ok && p > 0 {
		port = p
	}

	if globalServer != nil {
		fmt.Printf("Server is already running on port %d.\n", globalServer.port)
		return nil
	}

	fmt.Printf("Starting Blender Sync Server on port %d...\n", port)

	// Start the server
	globalServer = NewServer(port)

	// Run in goroutine to allow TUI to continue
	go func() {
		if err := globalServer.Start(); err != nil {
			// In a real app we might log this or handle restart
			globalServer = nil // reset on error
		}
	}()

	fmt.Printf("Server listening on :%d\n", port)
	fmt.Println("Run the script in Blender to sync geometry.")

	return nil
}
