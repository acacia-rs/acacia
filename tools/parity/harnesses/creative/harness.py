"""The creative inventory: CreativeModeInventoryScreen's tabs, item grid, scroll bar, search box, inventory tab with the player figure, and tooltips as GuiGraphics draw calls."""
from gui import guiparity

OWNER = "gui (Java look GUI)"


def run(h):
    guiparity.run(h, "acacia.parity.gui.CreativeOracle", "--count", 2400, probe_input="probe.jsonl")
