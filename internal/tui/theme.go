package tui

import "github.com/charmbracelet/lipgloss"

// TokyoNight Storm color palette
const (
	// Base colors
	bgColor     = "#24283b" // Dark blue-gray background
	fgColor     = "#c0caf5" // Light blue-gray foreground
	bgHighlight = "#292e42" // Slightly lighter background
	border      = "#414868" // Border color
	comment     = "#565f89" // Dimmed text

	// Accent colors
	red     = "#f7768e" // Salmon/coral pink
	green   = "#9ece6a" // Lime green
	yellow  = "#e0af68" // Yellow/tan
	blue    = "#7aa2f7" // Light blue
	magenta = "#bb9af7" // Purple/lavender
	cyan    = "#7dcfff" // Cyan
	orange  = "#ff9e64" // Orange

	// UI specific
	selection = "#364a82" // Selection background
	highlight = cyan      // Highlighted items
	title     = magenta   // Titles
	success   = green     // Success messages
	warning   = yellow    // Warnings
	error     = red       // Errors
)

// TokyoNight Storm themed styles
var (
	// Base styles
	BaseStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color(fgColor)).
			Background(lipgloss.Color(bgColor))

	// Title style - uses heavy border
	TitleStyle = lipgloss.NewStyle().
			Bold(true).
			Foreground(lipgloss.Color(title)).
			Background(lipgloss.Color(bgColor)).
			Padding(0, 1)

	// Breadcrumb/subtitle style
	BreadcrumbStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color(comment)).
			Background(lipgloss.Color(bgColor))

	// Selected item style - no bold to avoid height shifts
	SelectedStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color(highlight)).
			Background(lipgloss.Color(selection))

	// Normal item style - no padding to avoid height shifts
	NormalStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color(fgColor)).
			Background(lipgloss.Color(bgColor))

	// Description style - matches normal style dimensions, no italic to avoid shifts
	DescriptionStyle = lipgloss.NewStyle().
				Foreground(lipgloss.Color(comment)).
				Background(lipgloss.Color(bgColor))

	// Panel border style - heavy rounded border
	PanelStyle = lipgloss.NewStyle().
			BorderStyle(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color(border)).
			Background(lipgloss.Color(bgColor)).
			Padding(1, 2)

	// Help text style
	HelpStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color(comment)).
			Background(lipgloss.Color(bgColor))

	// Preview panel title
	PreviewTitleStyle = lipgloss.NewStyle().
				Bold(true).
				Foreground(lipgloss.Color(blue)).
				Background(lipgloss.Color(bgColor))

	// Stats/metadata style
	StatsStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color(green)).
			Background(lipgloss.Color(bgColor))

	// Window style (filled with colored blocks)
	WindowStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color(orange))

	// Wall style
	WallStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color(fgColor))

	// Input field style
	InputStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color(fgColor)).
			Background(lipgloss.Color(bgHighlight)).
			Padding(0, 1).
			BorderStyle(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color(border))

	// Label style for input fields
	LabelStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color(yellow)).
			Background(lipgloss.Color(bgColor)).
			Bold(true)

	// Error message style
	ErrorStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color(error)).
			Background(lipgloss.Color(bgColor)).
			Bold(true)

	// Success message style
	SuccessStyle = lipgloss.NewStyle().
			Foreground(lipgloss.Color(success)).
			Background(lipgloss.Color(bgColor)).
			Bold(true)
)

// GetColorForIndex returns a color from the palette by index (for visualization)
func GetColorForIndex(index int) string {
	colors := []string{red, green, yellow, blue, magenta, cyan, orange}
	return colors[index%len(colors)]
}
