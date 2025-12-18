package tui

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"math"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/charmbracelet/bubbles/key"
	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/winteweken/makit/internal/registry"
	"github.com/winteweken/makit/pkg/canvas"
	"github.com/winteweken/makit/pkg/geometry"
)

type Face struct {
	Points []geometry.Point
	Type   string // "wall_face" or "window_face"
}

type TreeItemType int

const (
	TypeSection TreeItemType = iota // "Sources", "Actions" headers
	TypeSource
	TypeCategory // For grouping Actions
	TypeAction
)

type TreeItem struct {
	Type     TreeItemType
	Name     string
	Level    int
	Expanded bool
	Source   *registry.Source // If TypeSource
	Action   *registry.Action // If TypeAction
	ID       string           // Unique ID for expansion tracking
}

type model struct {
	registry *registry.Registry
	cursor   int
	width    int
	height   int

	// Tree View State
	treeItems []TreeItem
	expanded  map[string]bool

	// Navigation state
	selectedSource *registry.Source
	activeSource   *registry.Source // The currently connected/active source context
	selectedAction *registry.Action
	// We keep selectedTask for backwards compatibility with methods that haven't been refactored or removed yet
	// But ideally we move away from it.
	// For options input, we need to know what we are configuring.
	activeContext interface{} // Can be *Source or *Action

	// Options input
	optionInputs []textinput.Model
	optionKeys   []string
	optionCursor int
	taskOptions  map[string]interface{}
	viewOptions  bool // True if we are in options input mode

	// Geometry preview
	previewLines []geometry.Line
	showPreview  bool // Toggles right panel visibility of preview

	// Task results
	lastTaskOutput    string
	showResults       bool
	resultsScroll     int // Scroll offset for results view
	vizDirections     []string
	selectedDirection int // Which direction to show in viz
	directionData     map[string]DirectionStats

	// 3D Logo rotation state
	logoRotationX float64
	logoRotationY float64

	// Focus State
	activePane int // 0 = Explorer (Left), 1 = Content (Right)

	// Keys
	keys keyMap
}

type DirectionStats struct {
	Walls      int
	Windows    int
	WallArea   float64
	WindowArea float64
	WWR        float64
	Faces      []Face
}

type keyMap struct {
	Up         key.Binding
	Down       key.Binding
	Left       key.Binding
	Right      key.Binding
	Enter      key.Binding
	Back       key.Binding
	Execute    key.Binding
	Quit       key.Binding
	Preview    key.Binding
	Results    key.Binding
	NextDir    key.Binding
	PrevDir    key.Binding
	SwitchPane key.Binding
}

func defaultKeyMap() keyMap {
	return keyMap{
		Up: key.NewBinding(
			key.WithKeys("up", "k"),
			key.WithHelp("↑/k", "up"),
		),
		Down: key.NewBinding(
			key.WithKeys("down", "j"),
			key.WithHelp("↓/j", "down"),
		),
		Left: key.NewBinding(
			key.WithKeys("left"),
			key.WithHelp("←", "prev direction"),
		),
		Right: key.NewBinding(
			key.WithKeys("right"),
			key.WithHelp("→", "next direction"),
		),
		Enter: key.NewBinding(
			key.WithKeys("enter", "l"),
			key.WithHelp("enter/l", "select"),
		),
		Back: key.NewBinding(
			key.WithKeys("esc", "h"),
			key.WithHelp("esc/h", "back"),
		),
		Execute: key.NewBinding(
			key.WithKeys("x"),
			key.WithHelp("x", "execute"),
		),
		SwitchPane: key.NewBinding(
			key.WithKeys("tab"),
			key.WithHelp("tab", "switch pane"),
		),
		Preview: key.NewBinding(
			key.WithKeys("p"),
			key.WithHelp("p", "toggle preview"),
		),
		Results: key.NewBinding(
			key.WithKeys("r"),
			key.WithHelp("r", "toggle results"),
		),
		NextDir: key.NewBinding(
			key.WithKeys("n", "]"),
			key.WithHelp("n/]", "next direction"),
		),
		PrevDir: key.NewBinding(
			key.WithKeys("b", "["),
			key.WithHelp("b/[", "prev direction"),
		),
		Quit: key.NewBinding(
			key.WithKeys("q", "ctrl+c"),
			key.WithHelp("q", "quit"),
		),
	}
}

func NewModel() model {
	// Ensure clean session by removing old visualization cache
	clearCache()

	m := model{
		registry:     registry.GetRegistry(),
		cursor:       0,
		keys:         defaultKeyMap(),
		showPreview:  false,
		showResults:  false,
		taskOptions:  make(map[string]interface{}),
		optionInputs: []textinput.Model{},
		expanded:     make(map[string]bool),
		treeItems:    []TreeItem{},
	}
	m.rebuildTree()
	return m
}

func clearCache() {
	files := []string{
		"/tmp/makit_viz.json",
		filepath.Join(os.TempDir(), "makit_viz.json"),
	}

	for _, f := range files {
		if _, err := os.Stat(f); err == nil {
			os.Remove(f)
		}
	}
}

func (m model) Init() tea.Cmd {
	return nil
}

// Logic for rebuildTree CDE:
// 1. Root: "Sources" (Expanded by default?)
// 2. Children: List all Sources
// 3. Root: "Actions" (Expanded by default?)
// 4. Children: Categories of Actions
// 5. Children: Actions

func (m *model) rebuildTree() {
	m.treeItems = []TreeItem{}

	// Sources Section
	m.treeItems = append(m.treeItems, TreeItem{
		Type:     TypeSection,
		Name:     "Sources",
		Level:    0,
		Expanded: m.expanded["section:sources"],
		ID:       "section:sources",
	})

	if m.expanded["section:sources"] {
		sources := m.registry.ListSources()
		for _, source := range sources {
			m.treeItems = append(m.treeItems, TreeItem{
				Type:     TypeSource,
				Name:     source.Name,
				Level:    1,
				Expanded: false, // Sources are leaves in this view (unless we show options in tree?)
				Source:   source,
				ID:       "source:" + source.Name,
			})
		}
	}

	// Actions Section
	m.treeItems = append(m.treeItems, TreeItem{
		Type:     TypeSection,
		Name:     "Actions",
		Level:    0,
		Expanded: m.expanded["section:actions"],
		ID:       "section:actions",
	})

	if m.expanded["section:actions"] {
		actions := m.registry.ListActions()

		// Group actions by Category
		categories := make(map[string][]*registry.Action)
		for _, action := range actions {
			cat := action.Category
			if cat == "" {
				cat = "General"
			}
			categories[cat] = append(categories[cat], action)
		}

		// Sort categories
		var sortedCats []string
		for cat := range categories {
			sortedCats = append(sortedCats, cat)
		}
		sort.Strings(sortedCats)

		for _, cat := range sortedCats {
			catID := "cat:" + cat
			m.treeItems = append(m.treeItems, TreeItem{
				Type:     TypeCategory,
				Name:     cat,
				Level:    1,
				Expanded: m.expanded[catID],
				ID:       catID,
			})

			if m.expanded[catID] {
				for _, action := range categories[cat] {
					m.treeItems = append(m.treeItems, TreeItem{
						Type:     TypeAction,
						Name:     action.Name,
						Level:    2,
						Expanded: false,
						Action:   action,
						ID:       "action:" + action.Name,
					})
				}
			}
		}
	}
}

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	// Handle options input mode separately
	if m.viewOptions {
		return m.updateOptionsView(msg)
	}

	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		return m, nil

	case tea.KeyMsg:
		// Handle global keys first (Quit, Tab)

		// Handle global keys first (Quit, Tab)
		if key.Matches(msg, m.keys.Quit) {
			return m, tea.Quit
		}
		if key.Matches(msg, m.keys.SwitchPane) {
			m.activePane = (m.activePane + 1) % 2
			return m, nil
		}

		// Dispatch based on active pane
		if m.activePane == 0 {
			// Explorer Pane (Left)
			switch {
			case key.Matches(msg, m.keys.Up):
				if m.cursor > 0 {
					m.cursor--
					m.updateSelectionFromCursor()
				}

			case key.Matches(msg, m.keys.Down):
				if m.cursor < len(m.treeItems)-1 {
					m.cursor++
					m.updateSelectionFromCursor()
				}

			case key.Matches(msg, m.keys.Enter), key.Matches(msg, m.keys.Right):
				item := m.treeItems[m.cursor]
				if item.Type == TypeSource || item.Type == TypeAction {
					m.showPreview = true
					m.previewLines = generateSampleGeometry(item.Name)
				} else {
					if !item.Expanded {
						m.expanded[item.ID] = true
						m.rebuildTree()
					}
				}

			case key.Matches(msg, m.keys.Back), key.Matches(msg, m.keys.Left):
				item := m.treeItems[m.cursor]
				if item.Expanded {
					m.expanded[item.ID] = false
					m.rebuildTree()
				} else {
					for i := m.cursor - 1; i >= 0; i-- {
						if m.treeItems[i].Level < item.Level {
							m.cursor = i
							m.updateSelectionFromCursor()
							break
						}
					}
				}

			case key.Matches(msg, m.keys.Execute):
				return m.handleExecute(), nil
			}

		} else {
			// Content Pane (Right)
			// Toggle between Results and Viz
			if key.Matches(msg, m.keys.Results) {
				m.showResults = !m.showResults
				if m.showResults {
					m.resultsScroll = 0
				}
				m.loadVisualizationData()
			}

			if m.showResults {
				// Scroll Results
				switch {
				case key.Matches(msg, m.keys.Up):
					if m.resultsScroll > 0 {
						m.resultsScroll--
					}
				case key.Matches(msg, m.keys.Down):
					m.resultsScroll++
				}
			} else {
				// Viz Interaction
				switch {
				case key.Matches(msg, m.keys.Left), key.Matches(msg, m.keys.PrevDir):
					if len(m.vizDirections) > 0 {
						m.selectedDirection--
						if m.selectedDirection < 0 {
							m.selectedDirection = len(m.vizDirections) - 1
						}
					} else {
						// Rotate 3D logo
						m.logoRotationY -= 0.15
					}
				case key.Matches(msg, m.keys.Right), key.Matches(msg, m.keys.NextDir):
					if len(m.vizDirections) > 0 {
						m.selectedDirection++
						if m.selectedDirection >= len(m.vizDirections) {
							m.selectedDirection = 0
						}
					} else {
						// Rotate 3D logo
						m.logoRotationY += 0.15
					}
				case key.Matches(msg, m.keys.Up):
					if len(m.vizDirections) == 0 {
						m.logoRotationX -= 0.15
					}
				case key.Matches(msg, m.keys.Down):
					if len(m.vizDirections) == 0 {
						m.logoRotationX += 0.15
					}
				}
			}
		}
	}

	return m, nil
}

func (m *model) updateSelectionFromCursor() {
	item := m.treeItems[m.cursor]

	// Reset cursor-based selection
	m.selectedSource = nil
	m.selectedAction = nil
	// activeContext follows the cursor to show options for *what is currently selected/hovered*
	m.activeContext = nil

	if item.Type == TypeSource {
		m.selectedSource = item.Source
		m.activeContext = item.Source
	} else if item.Type == TypeAction {
		m.selectedAction = item.Action
		m.activeContext = item.Action
	}

	// Note: We no longer show sample geometry on selection.
	// Geometry preview only appears after executing a task.
}

func (m model) handleExecute() model {
	var options []registry.TaskOption

	if m.selectedSource != nil {
		options = m.selectedSource.Options
	} else if m.selectedAction != nil {
		options = m.selectedAction.Options
	} else {
		return m
	}

	// Check if task has options
	if len(options) > 0 {
		// Switch to options view
		m.viewOptions = true
		m.optionCursor = 0
		m.setupOptionInputs()
		return m
	}

	// No options, execute directly

	output := m.executeTaskWithCapture(make(map[string]interface{}))
	m.lastTaskOutput = output
	m.showResults = true
	m.showPreview = true
	m.resultsScroll = 0 // Reset scroll for new results
	return m
}

func (m *model) setupOptionInputs() {
	m.optionInputs = []textinput.Model{}
	m.optionKeys = []string{}

	var options []registry.TaskOption
	if m.selectedSource != nil {
		options = m.selectedSource.Options
	} else if m.selectedAction != nil {
		options = m.selectedAction.Options
	}

	for _, opt := range options {
		ti := textinput.New()
		ti.Placeholder = opt.Description
		ti.CharLimit = 256

		// Set default value
		if opt.Default != nil {
			ti.SetValue(fmt.Sprintf("%v", opt.Default))
		}

		// Focus first input
		if len(m.optionInputs) == 0 {
			ti.Focus()
		}

		m.optionInputs = append(m.optionInputs, ti)
		m.optionKeys = append(m.optionKeys, opt.Name)
	}
}

func (m model) updateOptionsView(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case "ctrl+c", "q":
			return m, tea.Quit

		case "esc":
			m.viewOptions = false
			m.optionCursor = 0
			return m, nil

		case "tab", "down":
			// Move to next input
			if m.optionCursor < len(m.optionInputs)-1 {
				m.optionInputs[m.optionCursor].Blur()
				m.optionCursor++
				m.optionInputs[m.optionCursor].Focus()
			}
			return m, nil

		case "shift+tab", "up":
			// Move to previous input
			if m.optionCursor > 0 {
				m.optionInputs[m.optionCursor].Blur()
				m.optionCursor--
				m.optionInputs[m.optionCursor].Focus()
			}
			return m, nil

		case "enter":
			// Collect options and execute
			opts := make(map[string]interface{})
			for i, key := range m.optionKeys {
				value := m.optionInputs[i].Value()
				if value != "" {
					opts[key] = value
				}
			}

			// Execute and capture output
			output := m.executeTaskWithCapture(opts)
			m.lastTaskOutput = output
			m.showResults = true
			m.showPreview = true
			m.resultsScroll = 0 // Reset scroll for new results

			// Reset and go back
			m.viewOptions = false
			m.optionCursor = 0
			m.taskOptions = make(map[string]interface{})
			return m, nil
		}
	}

	// Update the focused input
	var cmd tea.Cmd
	m.optionInputs[m.optionCursor], cmd = m.optionInputs[m.optionCursor].Update(msg)
	return m, cmd
}

func (m *model) executeTaskWithCapture(options map[string]interface{}) string {
	// Capture stdout and stderr
	oldStdout := os.Stdout
	oldStderr := os.Stderr
	r, w, _ := os.Pipe()
	os.Stdout = w
	os.Stderr = w

	// Buffer to capture output
	outputChan := make(chan string)
	go func() {
		var buf bytes.Buffer
		io.Copy(&buf, r)
		outputChan <- buf.String()
	}()

	// Execute task
	ctx := &registry.TaskContext{
		Options: options,
	}

	var err error
	var name string

	if m.selectedSource != nil {
		// Connecting/Selecting a source
		name = m.selectedSource.Name

		fmt.Printf("Connecting to source: %s...\n", name)
		err = m.selectedSource.Handler(ctx)

		if err == nil {
			m.activeSource = m.selectedSource
			fmt.Printf("Successfully connected to %s.\n", name)
		}
	} else if m.selectedAction != nil {
		name = m.selectedAction.Name

		// If we have an active source, pass it in options for now until TaskContext supports it natively
		if m.activeSource != nil {
			if ctx.Options == nil {
				ctx.Options = make(map[string]interface{})
			}
			ctx.Options["_source"] = m.activeSource.Name
			fmt.Printf("Executing action '%s' on source '%s'...\n", name, m.activeSource.Name)
		} else {
			fmt.Printf("Executing action '%s' (No source connected)...\n", name)
		}

		err = m.selectedAction.Handler(ctx)
	}

	// Restore stdout/stderr
	w.Close()
	os.Stdout = oldStdout
	os.Stderr = oldStderr
	output := <-outputChan

	if err != nil {
		output += fmt.Sprintf("\n\nError: %v", err)
	}

	// Try to load visualization data if applicable
	// For sources like "blender", we assume it might trigger viz update
	if name == "start-server" || name == "wall-orientation-wwr" {
		m.loadVisualizationData()
	}
	// Generally try to reload viz data after any execution?
	// m.loadVisualizationData()

	return output
}

func (m *model) loadVisualizationData() {
	// Check standard temp dir first
	vizFile := filepath.Join(os.TempDir(), "makit_viz.json")

	data, err := os.ReadFile(vizFile)
	if err != nil {
		// Fallback to /tmp/makit_viz.json (hardcoded in analyze_ifc.py)
		vizFile = "/tmp/makit_viz.json"
		data, err = os.ReadFile(vizFile)
		if err != nil {
			return // Visualization not available
		}
	}

	var vizData map[string]interface{}
	if err := json.Unmarshal(data, &vizData); err != nil {
		return
	}

	// Check if data has faces format (check first direction)
	hasFaces := false
	for _, dirData := range vizData {
		if dirMap, ok := dirData.(map[string]interface{}); ok {
			if _, ok := dirMap["faces"]; ok {
				hasFaces = true
				break
			}
		}
	}

	// If data has faces, use the faces loading path
	if hasFaces {
		m.loadIsometricFaces(vizData)
		return
	}

	// Collect all lines from all directions
	type directionData struct {
		name  string
		lines []geometry.Line
	}
	var directions []directionData

	for direction, dirData := range vizData {
		dirMap, ok := dirData.(map[string]interface{})
		if !ok {
			continue
		}

		linesData, ok := dirMap["lines"].([]interface{})
		if !ok {
			continue
		}

		dirLines := []geometry.Line{}
		for _, lineData := range linesData {
			lineMap, ok := lineData.(map[string]interface{})
			if !ok {
				continue
			}

			x1 := lineMap["x1"].(float64)
			y1 := lineMap["y1"].(float64)
			x2 := lineMap["x2"].(float64)
			y2 := lineMap["y2"].(float64)

			dirLines = append(dirLines, geometry.Line{
				Start: geometry.Point{X: x1, Y: y1},
				End:   geometry.Point{X: x2, Y: y2},
			})
		}

		if len(dirLines) > 0 {
			directions = append(directions, directionData{
				name:  direction,
				lines: dirLines,
			})
		}
	}

	// Store directions separately for navigation (like faces mode)
	m.directionData = make(map[string]DirectionStats)
	m.vizDirections = []string{}

	for _, dir := range directions {
		// Store lines for this direction as a pseudo-face representation
		// Convert lines to a single "elevation" that can be navigated
		m.vizDirections = append(m.vizDirections, dir.name)

		// Count walls (each wall has 4 lines forming a rectangle)
		wallCount := len(dir.lines) / 4

		// Store direction data with lines converted to faces for consistent rendering
		faces := m.convertLinesToFaces(dir.lines)

		m.directionData[dir.name] = DirectionStats{
			Walls:   wallCount,
			Windows: 0, // Will be calculated if window data available
			WWR:     0.0,
			Faces:   faces,
		}
	}

	m.selectedDirection = 0
	m.previewLines = []geometry.Line{} // Clear, we'll use faces rendering
}

func (m *model) convertLinesToFaces(lines []geometry.Line) []Face {
	// Convert lines (4 per rectangle) to Face objects
	// Lines format: bottom, right, top, left for each rectangle
	faces := []Face{}

	for i := 0; i+3 < len(lines); i += 4 {
		// Get 4 lines forming a rectangle
		// Extract unique points from the 4 lines
		points := []geometry.Point{
			lines[i].Start,   // bottom-left
			lines[i].End,     // bottom-right
			lines[i+2].Start, // top-right
			lines[i+3].Start, // top-left
		}

		// Determine if this is a window or wall based on line type
		faceType := "wall_face"
		if i < len(lines) && len(lines) > 0 {
			// Simple heuristic: smaller rectangles are likely windows
			width := lines[i].End.X - lines[i].Start.X
			height := lines[i+1].End.Y - lines[i+1].Start.Y
			if width < 1000 || height < 200 { // Adjust thresholds as needed
				faceType = "window_face"
			}
		}

		faces = append(faces, Face{
			Points: points,
			Type:   faceType,
		})
	}

	return faces
}

func (m *model) loadIsometricFaces(vizData map[string]interface{}) {
	// Load direction-grouped visualization data
	m.directionData = make(map[string]DirectionStats)
	m.vizDirections = []string{}

	for dirName, dirDataInterface := range vizData {
		dirData, ok := dirDataInterface.(map[string]interface{})
		if !ok {
			continue
		}

		facesData, ok := dirData["faces"].([]interface{})
		if !ok {
			continue
		}

		// Parse faces for this direction
		dirFaces := []Face{}
		windowCount := 0

		for _, faceData := range facesData {
			faceMap, ok := faceData.(map[string]interface{})
			if !ok {
				continue
			}

			pointsData, ok := faceMap["points"].([]interface{})
			if !ok {
				continue
			}

			faceType, _ := faceMap["type"].(string)

			// Parse points
			points := []geometry.Point{}
			for _, pointData := range pointsData {
				pointSlice, ok := pointData.([]interface{})
				if !ok || len(pointSlice) < 2 {
					continue
				}

				x, _ := pointSlice[0].(float64)
				y, _ := pointSlice[1].(float64)
				z := 0.0
				if len(pointSlice) > 2 {
					z, _ = pointSlice[2].(float64)
				}

				// Apply Isometric Projection
				// x_iso = (x - y) * cos(30)
				// y_iso = (x + y) * sin(30) - z
				angle := 30.0 * math.Pi / 180.0
				cosAngle := math.Cos(angle)
				sinAngle := math.Sin(angle)

				isoX := (x - y) * cosAngle
				isoY := (x+y)*sinAngle - z

				points = append(points, geometry.Point{isoX, isoY})
			}

			if len(points) > 0 {
				dirFaces = append(dirFaces, Face{
					Points: points,
					Type:   faceType,
				})

				if faceType == "window_face" {
					windowCount++
				}
			}
		}

		// Get stats
		stats, _ := dirData["stats"].(map[string]interface{})
		wallCount, _ := stats["walls"].(float64)
		wwr, _ := stats["wwr"].(float64)

		// Normalize faces
		if len(dirFaces) > 0 {
			dirFaces = normalizeFaces(dirFaces, 100, 100)
		}

		// Store direction data
		m.directionData[dirName] = DirectionStats{
			Walls:   int(wallCount),
			Windows: windowCount,
			WWR:     wwr,
			Faces:   dirFaces,
		}
		m.vizDirections = append(m.vizDirections, dirName)
	}

	// Sort directions to ensure consistent order
	sort.Strings(m.vizDirections)

	// Move "Overview" to the first position if it exists
	overviewIndex := -1
	for i, dir := range m.vizDirections {
		if dir == "Overview" {
			overviewIndex = i
			break
		}
	}

	if overviewIndex > 0 {
		// Move Overview to front
		// Remove from current position
		m.vizDirections = append(m.vizDirections[:overviewIndex], m.vizDirections[overviewIndex+1:]...)
		// Prepend
		m.vizDirections = append([]string{"Overview"}, m.vizDirections...)
	}

	m.selectedDirection = 0
	m.previewLines = []geometry.Line{} // Clear lines
}

func normalizeFaces(faces []Face, targetWidth, targetHeight float64) []Face {
	if len(faces) == 0 {
		return faces
	}

	// Find bounds
	// Find bounds
	minX, maxX := faces[0].Points[0].X, faces[0].Points[0].X
	minY, maxY := faces[0].Points[0].Y, faces[0].Points[0].Y

	for _, face := range faces {
		for _, p := range face.Points {
			if p.X < minX {
				minX = p.X
			}
			if p.X > maxX {
				maxX = p.X
			}
			if p.Y < minY {
				minY = p.Y
			}
			if p.Y > maxY {
				maxY = p.Y
			}
		}
	}

	width := maxX - minX
	height := maxY - minY

	// Calculate scale
	scaleX := targetWidth / width
	scaleY := targetHeight / height
	scale := scaleX
	if scaleY < scaleX {
		scale = scaleY
	}

	// Normalize
	normalized := make([]Face, len(faces))
	for i, face := range faces {
		normalizedPoints := make([]geometry.Point, len(face.Points))
		for i, p := range face.Points {
			normalizedPoints[i] = geometry.Point{
				(p.X - minX) * scale,
				(p.Y - minY) * scale,
			}
		}
		normalized[i] = Face{
			Points: normalizedPoints,
			Type:   face.Type,
		}
	}

	return normalized
}

func (m model) View() string {
	if m.width == 0 {
		return "Loading..."
	}

	// Options view is full-width
	if m.viewOptions {
		return m.renderOptionsView()
	}

	leftPanel := m.renderLeftPanel()
	rightPanel := m.renderRightPanel()

	// Split view: left panel for navigation, right for preview
	leftWidth := m.width / 3 // Sidebar is 1/3
	rightWidth := m.width - leftWidth

	// Determine active pane border color
	leftBorderColor := lipgloss.Color(border)
	rightBorderColor := lipgloss.Color(border)

	if m.activePane == 0 {
		leftBorderColor = lipgloss.Color(highlight)
	} else {
		rightBorderColor = lipgloss.Color(highlight)
	}

	// Use TokyoNight themed panel styles with thick borders
	leftStyle := lipgloss.NewStyle().
		Width(leftWidth-2).
		Height(m.height-4).
		BorderStyle(lipgloss.ThickBorder()).
		BorderForeground(leftBorderColor).
		Background(lipgloss.Color(bgColor)).
		Foreground(lipgloss.Color(fgColor)).
		Padding(1, 1) // reduced padding for tree

	rightStyle := lipgloss.NewStyle().
		Width(rightWidth-2).
		Height(m.height-4).
		BorderStyle(lipgloss.ThickBorder()).
		BorderForeground(rightBorderColor).
		Background(lipgloss.Color(bgColor)).
		Foreground(lipgloss.Color(fgColor)).
		Padding(1, 2)

	// Render panels
	left := leftStyle.Render(leftPanel)
	right := rightStyle.Render(rightPanel)

	// Join horizontally
	content := lipgloss.JoinHorizontal(lipgloss.Top, left, right)

	// Add help footer with TokyoNight colors
	help := m.renderHelp()
	helpStyle := HelpStyle.Width(m.width)

	return lipgloss.JoinVertical(lipgloss.Left, content, helpStyle.Render(help))
}

func (m model) renderLeftPanel() string {
	var sb strings.Builder

	sb.WriteString(TitleStyle.Render("Explorer") + "\n\n")

	// Calculate visible window for scrolling logic if we had it
	// For now, render all and let lipgloss handle clipping or basic slice

	// Create window around cursor for scrolling
	visibleItems := m.height - 8 // Rough estimate of rows available
	if visibleItems < 1 {
		visibleItems = 1
	}

	start := 0
	end := len(m.treeItems)

	// Simple scroll logic
	if len(m.treeItems) > visibleItems {
		if m.cursor < visibleItems/2 {
			start = 0
			end = visibleItems
		} else if m.cursor >= len(m.treeItems)-visibleItems/2 {
			start = len(m.treeItems) - visibleItems
			end = len(m.treeItems)
		} else {
			start = m.cursor - visibleItems/2
			end = m.cursor + visibleItems/2
		}
	}

	if start < 0 {
		start = 0
	}
	if end > len(m.treeItems) {
		end = len(m.treeItems)
	}

	for i := start; i < end; i++ {
		item := m.treeItems[i]

		// Indentation
		indent := strings.Repeat("  ", item.Level)

		// Icon
		icon := ""
		if item.Type == TypeSource || item.Type == TypeAction {
			// Leaves
			if item.Type == TypeSource {
				icon = "⚡" // Bolt for source?
			} else {
				icon = "𝑓 " // function f for action
			}
		} else {
			// Containers
			if item.Expanded {
				icon = "▼ "
			} else {
				icon = "▶ "
			}
		}

		// Selection style
		cursor := " "
		lineStyle := NormalStyle
		if i == m.cursor {
			cursor = ">"
			lineStyle = SelectedStyle.Bold(true)
		}

		line := fmt.Sprintf("%s%s%s %s", cursor, indent, icon, item.Name)
		sb.WriteString(lineStyle.Render(line) + "\n")
	}

	return sb.String()
}

func (m model) renderOptionsView() string {
	var sb strings.Builder

	name := "Item"
	if m.selectedSource != nil {
		name = m.selectedSource.Name
	} else if m.selectedAction != nil {
		name = m.selectedAction.Name
	}

	sb.WriteString(TitleStyle.Padding(1, 2).Render(fmt.Sprintf("Configure: %s", name)))
	sb.WriteString("\n\n")

	for i, input := range m.optionInputs {
		if i < len(m.optionKeys) {
			label := LabelStyle.Width(20).Render(m.optionKeys[i] + ":")
			sb.WriteString(NormalStyle.Padding(0, 2).Render(label + " " + input.View()))
			sb.WriteString("\n\n")
		}
	}

	sb.WriteString("\n")
	sb.WriteString(HelpStyle.Render(m.renderHelp()))

	return sb.String()
}

func (m model) renderRightPanel() string {
	var sb strings.Builder

	// Render Tabs
	tabResults := " [ Results (r) ] "
	tabViz := " [ Visuals (p) ] "

	if m.showResults {
		tabResults = SelectedStyle.Render(tabResults)
		tabViz = BaseStyle.Render(tabViz)
	} else {
		tabResults = BaseStyle.Render(tabResults)
		tabViz = SelectedStyle.Render(tabViz)
	}

	sb.WriteString(tabResults + tabViz + "\n\n")

	// Show results text when toggled on
	if m.showResults && m.lastTaskOutput != "" {
		sb.WriteString(PreviewTitleStyle.Render("Analysis Results (↑/↓ to scroll)") + "\n\n")
		// ... remaining logic
		// Split output into lines and apply scroll offset
		lines := strings.Split(m.lastTaskOutput, "\n")

		// Clamp scroll position using local variable (View should not modify model)
		scrollPos := m.resultsScroll
		if scrollPos >= len(lines) {
			scrollPos = len(lines) - 1
		}
		if scrollPos < 0 {
			scrollPos = 0
		}

		// Calculate visible lines based on panel height
		maxVisibleLines := m.height - 10 // Account for borders, padding, title
		if maxVisibleLines < 1 {
			maxVisibleLines = 10
		}

		endLine := scrollPos + maxVisibleLines
		if endLine > len(lines) {
			endLine = len(lines)
		}

		visibleLines := lines[scrollPos:endLine]
		sb.WriteString(NormalStyle.Render(strings.Join(visibleLines, "\n")))

		// Show scroll indicator
		if len(lines) > maxVisibleLines {
			sb.WriteString(fmt.Sprintf("\n\n%s", HelpStyle.Render(
				fmt.Sprintf("Line %d-%d of %d", scrollPos+1, endLine, len(lines)))))
		}

		return sb.String()
	}

	// Show direction-specific elevation with faces
	// ... (geometry preview same as before)
	// To minimize complexity in replacement, we reuse the previous geometry rendering logic if m.showPreview is set

	if m.showPreview && len(m.previewLines) > 0 {
		previewWidth := m.width/2 - 6
		previewHeight := m.height - 8
		if previewWidth < 10 {
			previewWidth = 10
		}
		if previewHeight < 10 {
			previewHeight = 10
		}

		c := canvas.NewCanvas(previewWidth, previewHeight)
		geometry.DrawLines(c, m.previewLines, previewWidth, previewHeight)

		var sb strings.Builder
		name := "Preview"
		if m.selectedSource != nil {
			name = m.selectedSource.Name
		}
		if m.selectedAction != nil {
			name = m.selectedAction.Name
		}

		sb.WriteString(PreviewTitleStyle.Render(name) + "\n\n")
		sb.WriteString(c.Render())

		desc := ""
		if m.selectedSource != nil {
			desc = m.selectedSource.Description
		}
		if m.selectedAction != nil {
			desc = m.selectedAction.Description
		}

		sb.WriteString("\n\n" + DescriptionStyle.Render(desc))

		return sb.String()
	}

	if len(m.directionData) > 0 {
		return m.renderViz()
	}

	// Render 3D rotating cube logo
	cubeRender := m.render3DLogo()
	sb.WriteString(cubeRender)
	sb.WriteString("\n\n")

	msg := "Select a Source to connect or an Action to execute.\n\nPress 'x' to execute/connect."
	if m.activeSource != nil {
		msg += fmt.Sprintf("\n\nConnected Source: %s", m.activeSource.Name)
	}
	sb.WriteString(DescriptionStyle.Render(msg))
	sb.WriteString("\n\n")
	sb.WriteString(HelpStyle.Render("↑/↓/←/→: rotate cube"))
	return sb.String()
}

// render3DLogo renders a cross-section of an SDFX box primitive
func (m model) render3DLogo() string {
	// Create demo SDF box
	box, err := geometry.CreateDemoBox()
	if err != nil {
		return "Error creating SDF"
	}

	// Render slice with current rotation
	params := geometry.SliceParams{
		RotationX: m.logoRotationX,
		RotationY: m.logoRotationY,
		Depth:     0, // Slice through center
	}

	return geometry.RenderSDFSlice(box, 30, 15, params)
}

func (m model) renderViz() string {
	if len(m.directionData) > 0 && len(m.vizDirections) > 0 {
		currentDir := m.vizDirections[m.selectedDirection]
		dirStats := m.directionData[currentDir]

		var sb strings.Builder
		sb.WriteString(PreviewTitleStyle.Render(fmt.Sprintf("%s (%d faces)", currentDir, len(dirStats.Faces))) + "\n\n")

		// 1. Render Geometry (Faces)
		// --------------------------
		previewWidth := m.width/2 - 6
		previewHeight := m.height - 10 // Leave room for stats
		if previewWidth < 10 {
			previewWidth = 10
		}
		if previewHeight < 10 {
			previewHeight = 10
		}

		// Convert faces to lines for drawing
		var lines []geometry.Line
		for _, face := range dirStats.Faces {
			if len(face.Points) < 2 {
				continue
			}
			for i := 0; i < len(face.Points); i++ {
				p1 := face.Points[i]
				p2 := face.Points[(i+1)%len(face.Points)] // Wrap around to close loop
				lines = append(lines, geometry.Line{Start: p1, End: p2})
			}
		}

		c := canvas.NewCanvas(previewWidth, previewHeight)
		if len(lines) > 0 {
			geometry.DrawLines(c, lines, previewWidth, previewHeight)
			sb.WriteString(c.Render())
			sb.WriteString("\n\n")
		} else {
			sb.WriteString(DescriptionStyle.Render("No geometry to display."))
			sb.WriteString("\n\n")
		}

		// 2. Add compass/key plan (Optional: maybe hide if Overview/Isometric?)
		// --------------------------
		// Only show compass for cardinal directions, maybe not for Overview/Isometric if strictly 3D?
		// But let's keep it for now as reference.
		// sb.WriteString(m.renderCompass(currentDir))
		// sb.WriteString("\n\n")

		// Wall stats (using normal style)
		sb.WriteString(NormalStyle.Render(fmt.Sprintf("□ Walls: %d  (%.1f%% coverage)\n", dirStats.Walls, 100.0-dirStats.WWR)))

		// Window stats
		sb.WriteString(WindowStyle.Render(fmt.Sprintf("■ Windows: %d  (%.1f%% WWR)", dirStats.Windows, dirStats.WWR)))

		sb.WriteString("\n\n")
		sb.WriteString(HelpStyle.Render("←/→ or [/]: change direction  •  r: text results"))

		return sb.String()
	}
	return ""
}

func (m model) renderCompass(currentDirection string) string {
	// Create a key plan with building outline and direction indicators
	var sb strings.Builder

	highlightStyle := lipgloss.NewStyle().Foreground(lipgloss.Color("#7dcfff")).Bold(true) // Bright cyan
	normalStyle := lipgloss.NewStyle().Foreground(lipgloss.Color("#565f89"))               // Dimmed
	buildingStyle := lipgloss.NewStyle().Foreground(lipgloss.Color("#9ece6a"))             // Green
	viewingStyle := lipgloss.NewStyle().Foreground(lipgloss.Color("#ff9e64"))              // Orange

	// Helper to render direction with highlight if current
	renderDir := func(dir string, label string) string {
		if dir == currentDirection {
			return highlightStyle.Render(label)
		}
		return normalStyle.Render(label)
	}

	// Helper to render viewing indicator
	viewIndicator := func(dir string) string {
		if dir == currentDirection {
			return viewingStyle.Render("◄")
		}
		return " "
	}

	// Build key plan with building and compass
	sb.WriteString(DescriptionStyle.Render("Key Plan:") + "\n\n")
	sb.WriteString("       " + renderDir("North", "N") + "\n")
	sb.WriteString("         " + viewIndicator("North") + "\n")
	sb.WriteString("   " + renderDir("Northwest", "NW") + "   " + renderDir("Northeast", "NE") + "\n")
	sb.WriteString(" " + viewIndicator("Northwest") + "  " + buildingStyle.Render("┌─────┐") + "  " + viewIndicator("Northeast") + "\n")
	sb.WriteString(renderDir("West", "W") + " " + viewIndicator("West") + " " + buildingStyle.Render("│") + "     " + buildingStyle.Render("│") + " " + viewIndicator("East") + " " + renderDir("East", "E") + "\n")
	sb.WriteString(" " + viewIndicator("Southwest") + "  " + buildingStyle.Render("└─────┘") + "  " + viewIndicator("Southeast") + "\n")
	sb.WriteString("   " + renderDir("Southwest", "SW") + "   " + renderDir("Southeast", "SE") + "\n")
	sb.WriteString("         " + viewIndicator("South") + "\n")
	sb.WriteString("       " + renderDir("South", "S") + "\n")

	sb.WriteString("\n" + viewingStyle.Render("◄") + " = " + DescriptionStyle.Render("viewing direction"))

	return sb.String()
}

func (m model) renderHelp() string {
	if m.viewOptions {
		return "  ↑/↓/tab: next field  •  enter: execute  •  esc: back  •  q: quit"
	}
	if m.showResults {
		return "  ↑/↓: scroll results  •  r: close results  •  q: quit"
	}
	return "  ↑/↓: move  •  enter/right: expand/preview  •  left: collapse  •  x: execute  •  r: results  •  q: quit"
}

// Generate sample geometry for demonstration
func generateSampleGeometry(taskName string) []geometry.Line {
	lines := []geometry.Line{}

	switch taskName {
	case "revit-extract-walls", "extract-walls":
		// Simple room outline
		lines = append(lines,
			geometry.Line{Start: geometry.Point{0, 0}, End: geometry.Point{20, 0}},
			geometry.Line{Start: geometry.Point{20, 0}, End: geometry.Point{20, 15}},
			geometry.Line{Start: geometry.Point{20, 15}, End: geometry.Point{0, 15}},
			geometry.Line{Start: geometry.Point{0, 15}, End: geometry.Point{0, 0}},
			// Interior wall
			geometry.Line{Start: geometry.Point{10, 0}, End: geometry.Point{10, 15}},
		)

	case "revit-extract-floors", "extract-floors":
		// Floor slab outline
		lines = append(lines,
			geometry.Line{Start: geometry.Point{0, 0}, End: geometry.Point{25, 0}},
			geometry.Line{Start: geometry.Point{25, 0}, End: geometry.Point{25, 20}},
			geometry.Line{Start: geometry.Point{25, 20}, End: geometry.Point{0, 20}},
			geometry.Line{Start: geometry.Point{0, 20}, End: geometry.Point{0, 0}},
		)

	case "revit-extract-rooms", "extract-rooms":
		// Multiple rooms
		lines = append(lines,
			// Room 1
			geometry.Line{Start: geometry.Point{0, 0}, End: geometry.Point{10, 0}},
			geometry.Line{Start: geometry.Point{10, 0}, End: geometry.Point{10, 10}},
			geometry.Line{Start: geometry.Point{10, 10}, End: geometry.Point{0, 10}},
			geometry.Line{Start: geometry.Point{0, 10}, End: geometry.Point{0, 0}},
			// Room 2
			geometry.Line{Start: geometry.Point{10, 0}, End: geometry.Point{20, 0}},
			geometry.Line{Start: geometry.Point{20, 0}, End: geometry.Point{20, 10}},
			geometry.Line{Start: geometry.Point{20, 10}, End: geometry.Point{10, 10}},
		)

	default:
		// Default shape (Box)
		lines = append(lines,
			geometry.Line{Start: geometry.Point{5, 5}, End: geometry.Point{15, 5}},
			geometry.Line{Start: geometry.Point{15, 5}, End: geometry.Point{15, 12}},
			geometry.Line{Start: geometry.Point{15, 12}, End: geometry.Point{5, 12}},
			geometry.Line{Start: geometry.Point{5, 12}, End: geometry.Point{5, 5}},
		)
	}

	return lines
}
