"""GUI widgets: buttons, cycle buttons, checkboxes, sliders, edit boxes, text widgets, tab bar, selection list and layouts as GuiGraphics draw calls."""
from gui import guiparity

OWNER = "gui (Java look GUI)"


def run(h):
    guiparity.run(h, "acacia.parity.gui.WidgetsOracle", "--count", 2600)
