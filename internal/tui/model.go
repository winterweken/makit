package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/bubbles/key"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/winteweken/makit/internal/registry"
	"github.com/winteweken/makit/pkg/canvas"
	"github.com/winteweken/makit/pkg/geometry"
)

type viewMode int

const (
	viewTools viewMode = iota
	viewCategories
	viewTasks
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

	// Geometry preview
	previewLines []geometry.Line
	showPreview  bool

	// Keys
	keys keyMap
}

type keyMap struct {
	Up       key.Binding
	Down     key.Binding
	Enter    key.Binding
	Back     key.Binding
	Execute  key.Binding
	Quit     key.Binding
	Preview  key.Binding
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
		Quit: key.NewBinding(
			key.WithKeys("q", "ctrl+c"),
			key.WithHelp("q", "quit"),
		),
	}
}

func NewModel() model {
	return model{
		registry:    registry.GetRegistry(),
		mode:        viewTools,
		cursor:      0,
		keys:        defaultKeyMap(),
		showPreview: false,
	}
}

func (m model) Init() tea.Cmd {
	return nil
}

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
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
			if m.cursor > 0 {
				m.cursor--
			}

		case key.Matches(msg, m.keys.Down):
			maxCursor := m.getMaxCursor()
			if m.cursor < maxCursor-1 {
				m.cursor++
			}

		case key.Matches(msg, m.keys.Enter):
			return m.handleEnter(), nil

		case key.Matches(msg, m.keys.Back):
			return m.handleBack(), nil

		case key.Matches(msg, m.keys.Execute):
			return m.handleExecute(), nil

		case key.Matches(msg, m.keys.Preview):
			m.showPreview = !m.showPreview
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
		if m.selectedTool != nil {
			categories := make([]*registry.Category, 0, len(m.selectedTool.Categories))
			for _, cat := range m.selectedTool.Categories {
				categories = append(categories, cat)
			}
			if m.cursor < len(categories) {
				m.selectedCategory = categories[m.cursor]
				m.mode = viewTasks
				m.cursor = 0
			}
		}

	case viewTasks:
		// Task selected - could execute or show details
		if m.selectedCategory != nil {
			tasks := make([]*registry.Task, 0, len(m.selectedCategory.Tasks))
			for _, task := range m.selectedCategory.Tasks {
				tasks = append(tasks, task)
			}
			if m.cursor < len(tasks) {
				m.selectedTask = tasks[m.cursor]
				// Generate sample geometry for preview
				m.previewLines = generateSampleGeometry(m.selectedTask.Name)
				m.showPreview = true
			}
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
	}

	return m
}

func (m model) handleExecute() model {
	if m.selectedTask != nil {
		ctx := &registry.TaskContext{
			Options: make(map[string]interface{}),
		}
		// Execute the task
		m.registry.ExecuteTask(
			m.selectedTool.Name,
			m.selectedCategory.Name,
			m.selectedTask.Name,
			ctx,
		)
	}
	return m
}

func (m model) View() string {
	if m.width == 0 {
		return "Loading..."
	}

	leftPanel := m.renderLeftPanel()
	rightPanel := m.renderRightPanel()

	// Split view: left panel for navigation, right for preview
	leftWidth := m.width / 2
	rightWidth := m.width - leftWidth

	leftStyle := lipgloss.NewStyle().
		Width(leftWidth - 2).
		Height(m.height - 4).
		BorderStyle(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("62")).
		Padding(1)

	rightStyle := lipgloss.NewStyle().
		Width(rightWidth - 2).
		Height(m.height - 4).
		BorderStyle(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("62")).
		Padding(1)

	// Render panels
	left := leftStyle.Render(leftPanel)
	right := rightStyle.Render(rightPanel)

	// Join horizontally
	content := lipgloss.JoinHorizontal(lipgloss.Top, left, right)

	// Add help footer
	help := m.renderHelp()
	helpStyle := lipgloss.NewStyle().
		Foreground(lipgloss.Color("241")).
		Width(m.width)

	return lipgloss.JoinVertical(lipgloss.Left, content, helpStyle.Render(help))
}

func (m model) renderLeftPanel() string {
	var sb strings.Builder

	titleStyle := lipgloss.NewStyle().
		Bold(true).
		Foreground(lipgloss.Color("205"))

	switch m.mode {
	case viewTools:
		sb.WriteString(titleStyle.Render("Tools") + "\n\n")
		tools := m.registry.ListTools()
		for i, tool := range tools {
			cursor := " "
			if i == m.cursor {
				cursor = ">"
			}
			sb.WriteString(fmt.Sprintf("%s %s\n", cursor, tool.Name))
			sb.WriteString(fmt.Sprintf("  %s\n\n", tool.Description))
		}

	case viewCategories:
		sb.WriteString(titleStyle.Render(fmt.Sprintf("Tool: %s", m.selectedTool.Name)) + "\n\n")
		categories := make([]*registry.Category, 0, len(m.selectedTool.Categories))
		for _, cat := range m.selectedTool.Categories {
			categories = append(categories, cat)
		}
		for i, cat := range categories {
			cursor := " "
			if i == m.cursor {
				cursor = ">"
			}
			sb.WriteString(fmt.Sprintf("%s %s\n", cursor, cat.Name))
			sb.WriteString(fmt.Sprintf("  %s\n\n", cat.Description))
		}

	case viewTasks:
		sb.WriteString(titleStyle.Render(fmt.Sprintf("%s > %s", m.selectedTool.Name, m.selectedCategory.Name)) + "\n\n")
		tasks := make([]*registry.Task, 0, len(m.selectedCategory.Tasks))
		for _, task := range m.selectedCategory.Tasks {
			tasks = append(tasks, task)
		}
		for i, task := range tasks {
			cursor := " "
			if i == m.cursor {
				cursor = ">"
			}
			sb.WriteString(fmt.Sprintf("%s %s\n", cursor, task.Name))
			sb.WriteString(fmt.Sprintf("  %s\n\n", task.Description))
		}
	}

	return sb.String()
}

func (m model) renderRightPanel() string {
	if !m.showPreview || len(m.previewLines) == 0 {
		return lipgloss.NewStyle().
			Foreground(lipgloss.Color("241")).
			Render("Select a task and press 'p' to preview geometry")
	}

	// Render geometry preview
	previewWidth := m.width/2 - 6
	previewHeight := m.height - 8

	c := canvas.NewCanvas(previewWidth, previewHeight)
	geometry.DrawLines(c, m.previewLines, previewWidth, previewHeight)

	var sb strings.Builder
	sb.WriteString(lipgloss.NewStyle().Bold(true).Render("Geometry Preview") + "\n\n")
	sb.WriteString(c.Render())
	sb.WriteString(fmt.Sprintf("\n%d elements", len(m.previewLines)))

	return sb.String()
}

func (m model) renderHelp() string {
	return "  ↑/↓: navigate  •  enter: select  •  esc: back  •  p: preview  •  x: execute  •  q: quit"
}

// Generate sample geometry for demonstration
func generateSampleGeometry(taskName string) []geometry.Line {
	lines := []geometry.Line{}

	switch taskName {
	case "extract-walls":
		// Simple room outline
		lines = append(lines,
			geometry.Line{geometry.Point{0, 0}, geometry.Point{20, 0}},
			geometry.Line{geometry.Point{20, 0}, geometry.Point{20, 15}},
			geometry.Line{geometry.Point{20, 15}, geometry.Point{0, 15}},
			geometry.Line{geometry.Point{0, 15}, geometry.Point{0, 0}},
			// Interior wall
			geometry.Line{geometry.Point{10, 0}, geometry.Point{10, 15}},
		)

	case "extract-floors":
		// Floor slab outline
		lines = append(lines,
			geometry.Line{geometry.Point{0, 0}, geometry.Point{25, 0}},
			geometry.Line{geometry.Point{25, 0}, geometry.Point{25, 20}},
			geometry.Line{geometry.Point{25, 20}, geometry.Point{0, 20}},
			geometry.Line{geometry.Point{0, 20}, geometry.Point{0, 0}},
		)

	case "extract-rooms":
		// Multiple rooms
		lines = append(lines,
			// Room 1
			geometry.Line{geometry.Point{0, 0}, geometry.Point{10, 0}},
			geometry.Line{geometry.Point{10, 0}, geometry.Point{10, 10}},
			geometry.Line{geometry.Point{10, 10}, geometry.Point{0, 10}},
			geometry.Line{geometry.Point{0, 10}, geometry.Point{0, 0}},
			// Room 2
			geometry.Line{geometry.Point{10, 0}, geometry.Point{20, 0}},
			geometry.Line{geometry.Point{20, 0}, geometry.Point{20, 10}},
			geometry.Line{geometry.Point{20, 10}, geometry.Point{10, 10}},
		)

	default:
		// Default shape
		lines = append(lines,
			geometry.Line{geometry.Point{5, 5}, geometry.Point{15, 5}},
			geometry.Line{geometry.Point{15, 5}, geometry.Point{15, 12}},
			geometry.Line{geometry.Point{15, 12}, geometry.Point{5, 12}},
			geometry.Line{geometry.Point{5, 12}, geometry.Point{5, 5}},
		)
	}

	return lines
}
