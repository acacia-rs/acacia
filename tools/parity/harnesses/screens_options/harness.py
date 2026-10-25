"""GUI options screens: OptionsScreen and its option-list pages as GuiGraphics draw calls."""
from gui import guiparity

OWNER = "gui (Java look GUI)"


def run(h):
    guiparity.run(h, "acacia.parity.gui.OptionsOracle", "--count", 2000)
