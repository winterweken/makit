package tui

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
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

type viewMode int

const (
	viewTools viewMode = iota
	viewCategories
	viewTasks
	viewOptions
)

type model struct {
	registry *registry.Registry
	mode     viewMode
	cursor   int
	width    int
	height   int

	// Navigation state
	selectedTool     *registry.Tool
	selectedCategory *registry.Category
	selectedTask     *registry.Task

	// Options input
	optionInputs []textinput.Model
	optionKeys   []string
	optionCursor int
	taskOptions  map[string]interface{}

	// Geometry preview
	previewLines []geometry.Line
	previewFaces []Face
	showPreview  bool

	// Task results
	lastTaskOutput    string
	showResults       bool
	resultsScroll     int // Scroll offset for results view
	vizDirections     []string
	vizTotalWalls     int
	vizWindowCount    int
	selectedDirection int // Which direction to show in viz
	directionData     map[string]DirectionStats

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
	Up      key.Binding
	Down    key.Binding
	Left    key.Binding
	Right   key.Binding
	Enter   key.Binding
	Back    key.Binding
	Execute key.Binding
	Quit    key.Binding
	Preview key.Binding
	Results key.Binding
	NextDir key.Binding
	PrevDir key.Binding
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
	return model{
		registry:     registry.GetRegistry(),
		mode:         viewTools,
		cursor:       0,
		keys:         defaultKeyMap(),
		showPreview:  false,
		showResults:  false,
		taskOptions:  make(map[string]interface{}),
		optionInputs: []textinput.Model{},
	}
}

func (m model) Init() tea.Cmd {
	return nil
}

// getSortedCategories returns categories sorted by name
func (m model) getSortedCategories() []*registry.Category {
	if m.selectedTool == nil {
		return nil
	}

	categories := make([]*registry.Category, 0, len(m.selectedTool.Categories))
	for _, cat := range m.selectedTool.Categories {
		categories = append(categories, cat)
	}

	sort.Slice(categories, func(i, j int) bool {
		return categories[i].Name < categories[j].Name
	})

	return categories
}

// getSortedTasks returns tasks sorted by name
func (m model) getSortedTasks() []*registry.Task {
	if m.selectedCategory == nil {
		return nil
	}

	tasks := make([]*registry.Task, 0, len(m.selectedCategory.Tasks))
	for _, task := range m.selectedCategory.Tasks {
		tasks = append(tasks, task)
	}

	sort.Slice(tasks, func(i, j int) bool {
		return tasks[i].Name < tasks[j].Name
	})

	return tasks
}

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	// Handle options input mode separately
	if m.mode == viewOptions {
		return m.updateOptionsView(msg)
	}

	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		return m, nil

	case tea.KeyMsg:
		switch {
		case key.Matches(msg, m.keys.Quit):
			return m, tea.Quit

		case key.Matches(msg, m.keys.Up):
			// If showing results, scroll instead of moving cursor
			if m.showResults {
				if m.resultsScroll > 0 {
					m.resultsScroll--
				}
			} else {
				if m.cursor > 0 {
					m.cursor--
				}
			}

		case key.Matches(msg, m.keys.Down):
			// If showing results, scroll instead of moving cursor
			if m.showResults {
				m.resultsScroll++
			} else {
				maxCursor := m.getMaxCursor()
				if maxCursor > 0 {
					m.cursor++
					// Clamp cursor to valid range
					if m.cursor >= maxCursor {
						m.cursor = maxCursor - 1
					}
				}
			}

		case key.Matches(msg, m.keys.Enter):
			return m.handleEnter(), nil

		case key.Matches(msg, m.keys.Back):
			return m.handleBack(), nil

		case key.Matches(msg, m.keys.Execute):
			return m.handleExecute(), nil

		case key.Matches(msg, m.keys.Preview):
			m.showPreview = !m.showPreview

		case key.Matches(msg, m.keys.Results):
			if m.lastTaskOutput != "" {
				m.showResults = !m.showResults
				// Reset scroll when toggling results
				if m.showResults {
					m.resultsScroll = 0
				}
			}

		case key.Matches(msg, m.keys.Left), key.Matches(msg, m.keys.PrevDir):
			if len(m.vizDirections) > 0 {
				m.selectedDirection--
				if m.selectedDirection < 0 {
					m.selectedDirection = len(m.vizDirections) - 1
				}
			}

		case key.Matches(msg, m.keys.Right), key.Matches(msg, m.keys.NextDir):
			if len(m.vizDirections) > 0 {
				m.selectedDirection++
				if m.selectedDirection >= len(m.vizDirections) {
					m.selectedDirection = 0
				}
			}
		}
	}

	return m, nil
}

func (m model) getMaxCursor() int {
	switch m.mode {
	case viewTools:
		return len(m.registry.Tools)
	case viewCategories:
		if m.selectedTool != nil {
			return len(m.selectedTool.Categories)
		}
	case viewTasks:
		if m.selectedCategory != nil {
			return len(m.selectedCategory.Tasks)
		}
	}
	return 0
}

func (m model) handleEnter() model {
	switch m.mode {
	case viewTools:
		tools := m.registry.ListTools()
		if m.cursor < len(tools) {
			m.selectedTool = tools[m.cursor]
			m.mode = viewCategories
			m.cursor = 0
		}

	case viewCategories:
		categories := m.getSortedCategories()
		if m.cursor < len(categories) {
			m.selectedCategory = categories[m.cursor]
			m.mode = viewTasks
			m.cursor = 0
		}

	case viewTasks:
		// Task selected - could execute or show details
		tasks := m.getSortedTasks()
		if m.cursor < len(tasks) {
			m.selectedTask = tasks[m.cursor]
			// Generate sample geometry for preview
			m.previewLines = generateSampleGeometry(m.selectedTask.Name)
			m.showPreview = true
		}
	}

	return m
}

func (m model) handleBack() model {
	switch m.mode {
	case viewCategories:
		m.mode = viewTools
		m.selectedTool = nil
		m.cursor = 0
		m.showPreview = false

	case viewTasks:
		m.mode = viewCategories
		m.selectedCategory = nil
		m.selectedTask = nil
		m.cursor = 0
		m.showPreview = false

	case viewOptions:
		m.mode = viewTasks
		m.optionCursor = 0
		m.taskOptions = make(map[string]interface{})
	}

	// Clear results when navigating back
	if m.mode == viewTools || m.mode == viewCategories {
		m.showResults = false
		m.lastTaskOutput = ""
	}

	return m
}

func (m model) handleExecute() model {
	if m.selectedTask == nil {
		return m
	}

	// Check if task has options
	if len(m.selectedTask.Options) > 0 {
		// Switch to options view
		m.mode = viewOptions
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

	for _, opt := range m.selectedTask.Options {
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
			m.mode = viewTasks
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
			m.mode = viewTasks
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
	err := m.registry.ExecuteTask(
		m.selectedTool.Name,
		m.selectedCategory.Name,
		m.selectedTask.Name,
		ctx,
	)

	// Restore stdout/stderr
	w.Close()
	os.Stdout = oldStdout
	os.Stderr = oldStderr
	output := <-outputChan

	if err != nil {
		output += fmt.Sprintf("\n\nError: %v", err)
	}

	// Try to load visualization data if this was an IFC analysis
	if m.selectedTask.Name == "wall-orientation-wwr" {
		m.loadVisualizationData()
	}

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
				points = append(points, geometry.Point{X: x, Y: y})
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
		for j, p := range face.Points {
			normalizedPoints[j] = geometry.Point{
				X: (p.X - minX) * scale,
				Y: (p.Y - minY) * scale,
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
	if m.mode == viewOptions {
		return m.renderOptionsView()
	}

	leftPanel := m.renderLeftPanel()
	rightPanel := m.renderRightPanel()

	// Split view: left panel for navigation, right for preview
	leftWidth := m.width / 2
	rightWidth := m.width - leftWidth

	// Use TokyoNight themed panel styles with thick borders
	// No fixed height - let content determine size naturally to prevent shifting
	leftStyle := lipgloss.NewStyle().
		Width(leftWidth-2).
		MaxHeight(m.height-4).
		BorderStyle(lipgloss.ThickBorder()).
		BorderForeground(lipgloss.Color(border)).
		Background(lipgloss.Color(bgColor)).
		Foreground(lipgloss.Color(fgColor)).
		Padding(1, 2)

	rightStyle := lipgloss.NewStyle().
		Width(rightWidth-2).
		MaxHeight(m.height-4).
		BorderStyle(lipgloss.ThickBorder()).
		BorderForeground(lipgloss.Color(border)).
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
	helpStyle := HelpStyle.Copy().Width(m.width)

	return lipgloss.JoinVertical(lipgloss.Left, content, helpStyle.Render(help))
}

func (m model) renderLeftPanel() string {
	var sb strings.Builder

	switch m.mode {
	case viewTools:
		sb.WriteString(TitleStyle.Render("Select Tool") + "\n")
		sb.WriteString(BreadcrumbStyle.Render("Step 1 of 3") + "\n\n")

		tools := m.registry.ListTools()
		for i, tool := range tools {
			cursor := " "
			if i == m.cursor {
				cursor = ">"
			}
			sb.WriteString(fmt.Sprintf("%s %d. %s\n", cursor, i+1, tool.Name))
			sb.WriteString(fmt.Sprintf("     %s\n\n", tool.Description))
		}

	case viewCategories:
		sb.WriteString(BreadcrumbStyle.Render(m.selectedTool.Name+" >") + " ")
		sb.WriteString(TitleStyle.Render("Select Category") + "\n")
		sb.WriteString(BreadcrumbStyle.Render("Step 2 of 3") + "\n\n")

		categories := m.getSortedCategories()
		for i, cat := range categories {
			cursor := " "
			if i == m.cursor {
				cursor = ">"
			}
			sb.WriteString(fmt.Sprintf("%s %d. %s\n", cursor, i+1, cat.Name))
			sb.WriteString(fmt.Sprintf("     %s\n\n", cat.Description))
		}

	case viewTasks:
		sb.WriteString(BreadcrumbStyle.Render(m.selectedTool.Name+" > "+m.selectedCategory.Name+" >") + " ")
		sb.WriteString(TitleStyle.Render("Select Task") + "\n")
		sb.WriteString(BreadcrumbStyle.Render("Step 3 of 3") + "\n\n")

		tasks := m.getSortedTasks()
		for i, task := range tasks {
			cursor := " "
			if i == m.cursor {
				cursor = ">"
			}
			sb.WriteString(fmt.Sprintf("%s %d. %s\n", cursor, i+1, task.Name))
			sb.WriteString(fmt.Sprintf("     %s\n\n", task.Description))
		}
	}

	return sb.String()
}

func (m model) renderRightPanel() string {
	// Show results text when toggled on
	if m.showResults && m.lastTaskOutput != "" {
		var sb strings.Builder

		sb.WriteString(PreviewTitleStyle.Render("Analysis Results (↑/↓ to scroll)") + "\n\n")

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
	if len(m.directionData) > 0 && len(m.vizDirections) > 0 {
		previewWidth := m.width/2 - 6
		previewHeight := m.height - 8

		// Get current direction
		currentDir := m.vizDirections[m.selectedDirection]
		dirStats := m.directionData[currentDir]

		if len(dirStats.Faces) == 0 {
			// No faces to render
			var sb strings.Builder
			sb.WriteString(PreviewTitleStyle.Render(currentDir) + "\n\n")
			sb.WriteString(DescriptionStyle.Render("No geometry data available"))
			return sb.String()
		}

		// Find overall bounds of all faces
		allMinX, allMaxX := dirStats.Faces[0].Points[0].X, dirStats.Faces[0].Points[0].X
		allMinY, allMaxY := dirStats.Faces[0].Points[0].Y, dirStats.Faces[0].Points[0].Y
		for _, face := range dirStats.Faces {
			for _, p := range face.Points {
				if p.X < allMinX {
					allMinX = p.X
				}
				if p.X > allMaxX {
					allMaxX = p.X
				}
				if p.Y < allMinY {
					allMinY = p.Y
				}
				if p.Y > allMaxY {
					allMaxY = p.Y
				}
			}
		}

		// Calculate scale to fit canvas (with padding)
		dataWidth := allMaxX - allMinX
		dataHeight := allMaxY - allMinY

		// Canvas dimensions in braille pixels (2 wide x 4 tall per character)
		canvasPixelWidth := float64(previewWidth * 2)
		canvasPixelHeight := float64(previewHeight * 4)

		// Add padding (10% on each side)
		padding := 0.9
		scaleX := (canvasPixelWidth * padding) / dataWidth
		scaleY := (canvasPixelHeight * padding) / dataHeight
		scale := scaleX
		if scaleY < scaleX {
			scale = scaleY
		}

		// Center offset
		offsetX := (canvasPixelWidth - dataWidth*scale) / 2
		offsetY := (canvasPixelHeight - dataHeight*scale) / 2

		// Create separate canvases for layering
		wallCanvas := canvas.NewCanvas(previewWidth, previewHeight)
		windowCanvas := canvas.NewCanvas(previewWidth, previewHeight)

		// Render faces for this direction
		for _, face := range dirStats.Faces {
			if len(face.Points) < 3 {
				continue
			}

			// Find bounding box
			minX, maxX := face.Points[0].X, face.Points[0].X
			minY, maxY := face.Points[0].Y, face.Points[0].Y
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

			// Scale and offset to canvas pixel coordinates
			x := int((minX-allMinX)*scale + offsetX)
			y := int((minY-allMinY)*scale + offsetY)
			w := int((maxX - minX) * scale)
			h := int((maxY - minY) * scale)

			// Ensure minimum size for visibility
			if w < 2 {
				w = 2
			}
			if h < 2 {
				h = 2
			}

			// Render based on type to separate canvases
			if face.Type == "window_face" {
				// Windows: filled rectangles on window layer
				windowCanvas.FillRect(x, y, w, h)
			} else if face.Type == "wall_face" {
				// Walls: outlined rectangles on wall layer
				wallCanvas.DrawRect(x, y, w, h)
			}
		}

		// Layer Merging: Combine canvases with colors
		wallStr := wallCanvas.Render()
		winStr := windowCanvas.Render()

		wallRows := strings.Split(wallStr, "\n")
		winRows := strings.Split(winStr, "\n")

		var mergedSb strings.Builder

		// Iterate rows (skip empty last split if any)
		for y := 0; y < len(wallRows) && y < len(winRows); y++ {
			if len(wallRows[y]) == 0 {
				continue
			}

			wallRow := []rune(wallRows[y])
			winRow := []rune(winRows[y])

			for x := 0; x < len(wallRow) && x < len(winRow); x++ {
				wChar := winRow[x]
				wallChar := wallRow[x]

				// 0x2800 is empty braille pattern (⠀)
				if wChar != 0x2800 {
					// Window pattern takes precedence and gets color
					mergedSb.WriteString(WindowStyle.Render(string(wChar)))
				} else if wallChar != 0x2800 {
					// Wall pattern gets standard color
					mergedSb.WriteString(WallStyle.Render(string(wallChar)))
				} else {
					// Empty space
					mergedSb.WriteRune(wallChar)
				}
			}
			mergedSb.WriteRune('\n')
		}

		canvasOutput := mergedSb.String()

		// Render with styled output
		var sb strings.Builder

		// Direction header with navigation
		dirHeader := fmt.Sprintf("◀ %s (%d/%d) ▶", currentDir, m.selectedDirection+1, len(m.vizDirections))
		sb.WriteString(PreviewTitleStyle.Render(dirHeader) + "\n\n")

		// Style the canvas with legend colors
		sb.WriteString(canvasOutput)

		// Stats and legend
		sb.WriteString("\n\n")

		// Wall stats (using normal style)
		sb.WriteString(NormalStyle.Render(fmt.Sprintf("□ Walls: %d  (%.1f%% coverage)\n", dirStats.Walls, 100.0-dirStats.WWR)))

		// Window stats (cyan colored to match filled appearance)
		sb.WriteString(WindowStyle.Render(fmt.Sprintf("■ Windows: %d  (%.1f%% WWR)", dirStats.Windows, dirStats.WWR)))

		sb.WriteString("\n\n")

		// Add compass/key plan
		sb.WriteString(m.renderCompass(currentDir))

		sb.WriteString("\n")
		sb.WriteString(HelpStyle.Render("←/→ or [/]: change direction  •  r: text results"))

		return sb.String()
	}

	// Show geometry visualization if available (elevation view)
	if len(m.previewLines) > 0 {
		previewWidth := m.width/2 - 6
		previewHeight := m.height - 8

		c := canvas.NewCanvas(previewWidth, previewHeight)
		geometry.DrawLines(c, m.previewLines, previewWidth, previewHeight)

		var sb strings.Builder
		sb.WriteString(PreviewTitleStyle.Render("Wall Elevation View") + "\n\n")
		sb.WriteString(c.Render())

		// Show metadata
		sb.WriteString(StatsStyle.Render(fmt.Sprintf("\n\n%d walls visualized", m.vizTotalWalls)))
		if len(m.vizDirections) > 0 {
			sb.WriteString(DescriptionStyle.Render(fmt.Sprintf("\nDirections: %s", strings.Join(m.vizDirections, ", "))))
		}
		sb.WriteString(HelpStyle.Render("\n\nPress 'r' to see analysis results"))

		return sb.String()
	}

	// Default message
	return DescriptionStyle.Render("Select a task and press 'x' to execute\n\nResults and visualizations will appear here")
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
	if m.mode == viewOptions {
		return "  ↑/↓/tab: next field  •  enter: execute  •  esc: back  •  q: quit"
	}
	if len(m.directionData) > 0 {
		return "  ↑/↓ or 1-9: navigate  •  ←/→: change direction  •  enter: select  •  esc: back  •  x: execute  •  r: results  •  q: quit"
	}
	if m.lastTaskOutput != "" {
		return "  ↑/↓ or 1-9: navigate  •  enter: select  •  esc: back  •  r: toggle results  •  x: execute  •  q: quit"
	}
	return "  ↑/↓ or 1-9: navigate  •  enter: select  •  esc: back  •  x: execute  •  q: quit"
}

func (m model) renderOptionsView() string {
	var sb strings.Builder

	sb.WriteString(TitleStyle.Copy().Padding(1, 2).Render(fmt.Sprintf("Configure: %s", m.selectedTask.Name)))
	sb.WriteString("\n\n")

	for i, input := range m.optionInputs {
		label := LabelStyle.Copy().Width(20).Render(m.optionKeys[i] + ":")
		sb.WriteString(NormalStyle.Copy().Padding(0, 2).Render(label + " " + input.View()))
		sb.WriteString("\n\n")
	}

	sb.WriteString("\n")
	sb.WriteString(HelpStyle.Render(m.renderHelp()))

	return sb.String()
}

// Generate sample geometry for demonstration
func generateSampleGeometry(taskName string) []geometry.Line {
	lines := []geometry.Line{}

	switch taskName {
	case "extract-walls":
		// Simple room outline
		lines = append(lines,
			geometry.Line{Start: geometry.Point{X: 0, Y: 0}, End: geometry.Point{X: 20, Y: 0}},
			geometry.Line{Start: geometry.Point{X: 20, Y: 0}, End: geometry.Point{X: 20, Y: 15}},
			geometry.Line{Start: geometry.Point{X: 20, Y: 15}, End: geometry.Point{X: 0, Y: 15}},
			geometry.Line{Start: geometry.Point{X: 0, Y: 15}, End: geometry.Point{X: 0, Y: 0}},
			// Interior wall
			geometry.Line{Start: geometry.Point{X: 10, Y: 0}, End: geometry.Point{X: 10, Y: 15}},
		)

	case "extract-floors":
		// Floor slab outline
		lines = append(lines,
			geometry.Line{Start: geometry.Point{X: 0, Y: 0}, End: geometry.Point{X: 25, Y: 0}},
			geometry.Line{Start: geometry.Point{X: 25, Y: 0}, End: geometry.Point{X: 25, Y: 20}},
			geometry.Line{Start: geometry.Point{X: 25, Y: 20}, End: geometry.Point{X: 0, Y: 20}},
			geometry.Line{Start: geometry.Point{X: 0, Y: 20}, End: geometry.Point{X: 0, Y: 0}},
		)

	case "extract-rooms":
		// Multiple rooms
		lines = append(lines,
			// Room 1
			geometry.Line{Start: geometry.Point{X: 0, Y: 0}, End: geometry.Point{X: 10, Y: 0}},
			geometry.Line{Start: geometry.Point{X: 10, Y: 0}, End: geometry.Point{X: 10, Y: 10}},
			geometry.Line{Start: geometry.Point{X: 10, Y: 10}, End: geometry.Point{X: 0, Y: 10}},
			geometry.Line{Start: geometry.Point{X: 0, Y: 10}, End: geometry.Point{X: 0, Y: 0}},
			// Room 2
			geometry.Line{Start: geometry.Point{X: 10, Y: 0}, End: geometry.Point{X: 20, Y: 0}},
			geometry.Line{Start: geometry.Point{X: 20, Y: 0}, End: geometry.Point{X: 20, Y: 10}},
			geometry.Line{Start: geometry.Point{X: 20, Y: 10}, End: geometry.Point{X: 10, Y: 10}},
		)

	default:
		// Default shape
		lines = append(lines,
			geometry.Line{Start: geometry.Point{X: 5, Y: 5}, End: geometry.Point{X: 15, Y: 5}},
			geometry.Line{Start: geometry.Point{X: 15, Y: 5}, End: geometry.Point{X: 15, Y: 12}},
			geometry.Line{Start: geometry.Point{X: 15, Y: 12}, End: geometry.Point{X: 5, Y: 12}},
			geometry.Line{Start: geometry.Point{X: 5, Y: 12}, End: geometry.Point{X: 5, Y: 5}},
		)
	}

	return lines
}
