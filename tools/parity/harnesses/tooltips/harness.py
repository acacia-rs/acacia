"""Tooltips: item tooltip lines (ItemStack.getTooltipLines) and tooltip drawing (positioners, styles, images) as GuiGraphics draw calls."""
from gui import guiparity

OWNER = "gui (Java look GUI)"


def run(h):
    guiparity.run(h, "acacia.parity.gui.TooltipOracle", "--count", 5200, probe_input="probe.jsonl")
