"""GUI text screens: sign and hanging-sign editors, book viewer and editor, telemetry info as GuiGraphics draw calls."""
from gui import guiparity

OWNER = "gui (Java look GUI)"


def run(h):
    guiparity.run(h, "acacia.parity.gui.TextOracle", "--count", 2400)
