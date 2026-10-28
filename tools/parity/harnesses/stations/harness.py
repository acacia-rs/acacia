"""Station screens: anvil, grindstone, smithing table, cartography table, enchanting table and beacon as GuiGraphics draw calls (windows, name box, costs, enchantment options, cycling slot icons, pictures, power buttons, tooltips)."""
from gui import guiparity

OWNER = "gui (Java look GUI)"


def run(h):
    guiparity.run(h, "acacia.parity.gui.StationOracle", "--count", 2400, probe_input="probe.jsonl")
