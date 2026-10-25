"""GUI loading and credits screens: level loading, generic message, credits and attribution, and the credits roll as GuiGraphics draw calls."""
from gui import guiparity

OWNER = "gui (Java look GUI)"


def run(h):
    guiparity.run(h, "acacia.parity.gui.MoreOracle", "--count", 2400)
