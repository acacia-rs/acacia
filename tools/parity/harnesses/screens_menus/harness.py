"""GUI menu screens: pause, death, confirm, alert, progress, disconnected and completed-waiting screens as GuiGraphics draw calls."""
from gui import guiparity

OWNER = "gui (Java look GUI)"


def run(h):
    guiparity.run(h, "acacia.parity.gui.MenusOracle", "--count", 2400)
