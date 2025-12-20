# Makit pyRevit Extension

This pyRevit extension provides an HTTP server that exposes Revit's API to external tools, enabling the Makit CLI/TUI to extract geometry and data from Revit models.

## Features

- HTTP API server running on `http://localhost:8888`
- Automatic startup with Revit
- Geometry extraction endpoints (walls, floors, rooms)
- Project information endpoints
- Real-time data access from active Revit model

## Installation

### Option 1: Copy to pyRevit Extensions Folder

1. Copy the `Makit.extension` folder to your pyRevit extensions directory:
   - Windows: `%APPDATA%\pyRevit\Extensions\`
   - Or use pyRevit's custom extension directories

2. Reload pyRevit:
   - Click the pyRevit tab in Revit
   - Click "Reload" or restart Revit

### Option 2: Link Extension (Development)

For development, you can create a symlink:

```bash
# Windows (run as Administrator)
mklink /D "%APPDATA%\pyRevit\Extensions\Makit.extension" "C:\path\to\makit\pyrevit-extension\Makit.extension"

# Or add the extension path in pyRevit settings
```

## Usage

### Automatic Startup

The HTTP server starts automatically when Revit loads. You'll see a message in the Revit status bar.

### Manual Control

Use the **Start Server** button in the **Makit** tab to manually start the server if needed.

### Testing the Connection

1. Open a Revit model
2. From a terminal, test the connection:

```bash
# Check server health
curl http://localhost:8888/health

# Get project info
curl http://localhost:8888/api/project/info

# Extract walls (using makit CLI)
makit exec revit geometry extract-walls --output walls.json
```

## API Endpoints

### Health Check
- **GET** `/health`
- Returns: `{"status": "ok", "revit_available": true}`

### Project Information
- **GET** `/api/project/info`
- Returns project metadata (name, number, version, etc.)

### Extract Walls
- **POST** `/api/geometry/walls`
- Body: `{"level": "Level 1", "includeCurved": true}`
- Returns: Wall elements with geometry

### Extract Floors
- **POST** `/api/geometry/floors`
- Body: `{"level": "Level 1"}`
- Returns: Floor elements with geometry

### Extract Rooms
- **POST** `/api/geometry/rooms`
- Body: `{"includeUnplaced": false}`
- Returns: Room elements with boundaries

## Development

### File Structure

```
Makit.extension/
├── extension.json          # Extension metadata
├── startup.py              # Auto-start script
├── lib/
│   ├── makit_server.py     # HTTP server implementation
│   └── geometry_extractor.py  # Revit API geometry extraction
└── Makit.tab/
    └── Server.panel/
        └── StartServer.pushbutton/
            └── script.py   # Manual server start button
```

### Adding New Endpoints

1. Add handler in `lib/makit_server.py`:
```python
elif self.path == '/api/your/endpoint':
    try:
        from geometry_extractor import your_function
        result = your_function(options)
        self._send_json(result)
    except Exception as e:
        self._send_error_json(str(e))
```

2. Implement function in `lib/geometry_extractor.py`:
```python
def your_function(options):
    # Use Revit API here
    return {'data': [...]}
```

3. Add corresponding Go client method in `internal/pyrevit/client.go`

## Troubleshooting

### Server Not Starting

- Check the pyRevit output window for errors
- Ensure port 8888 is not in use by another application
- Reload pyRevit extension

### Connection Refused from Makit CLI

- Verify Revit is running with the extension loaded
- Check if server is running: `curl http://localhost:8888/health`
- Make sure no firewall is blocking localhost connections

### Empty Geometry Results

- Ensure a Revit model is open (not just Revit application)
- Check that elements exist in the model
- Verify element categories match the extraction filters

## Security Note

This server runs on localhost only and is intended for local development. Do not expose it to external networks.

## License

MIT License - See LICENSE file in the root repository
