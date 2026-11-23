# The README's TUI recording. Render from the repository root with:
#   vhs docs/media/makit-tui.tape
# Needs: the local sample Ifc4_ARC.ifc in examples/IFC, the Toronto EPW in
# examples/EPW, a Python with IfcOpenShell in .venv, and murb's pure-Python
# branch checked out at ../murb-pp. The IFC is touched first so the picker
# shows extraction on demand; move any murb_results.json out of the root
# first so the TUI starts without cached results.

Output docs/media/makit-tui.gif

Set Shell zsh
Set FontSize 14
Set Width 1120
Set Height 740
Set Padding 12
Set Framerate 20
Set TypingSpeed 30ms

Hide
Type "export PYTHONPATH=../murb-pp MAKIT_PYTHON=.venv/bin/python; touch examples/IFC/Ifc4_ARC.ifc; cargo build -q -p makit; clear" Enter
Sleep 2s
Show

Type "cargo run -q -p makit -- tui" Enter
Sleep 2500ms

# Select the ifc source: its file picker appears in the Detail pane.
Down 2
Sleep 1200ms

# Load the Revit sample house: it is extracted first, then shown.
Tab
Sleep 600ms
Down 2
Sleep 800ms
Enter
Sleep 5s

# Back to the Explorer, down to analysis › murb-simulate.
Shift+Tab
Sleep 600ms
Down 5
Sleep 500ms
Right
Sleep 500ms
Down
Sleep 1200ms

# Run it: only the weather file is asked for; the model comes from the picker.
Type "e"
Sleep 1200ms
Type "examples/EPW/Toronto_TMU/CAN_ON_Toronto.City-Univ.of.Toronto.715080_TMYx.epw"
Sleep 600ms
Enter
Sleep 6s

# The help overlay, then out.
Type "?"
Sleep 2500ms
Escape
Sleep 1500ms
Type "q"
Sleep 600ms
