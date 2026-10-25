"""Container screens: AbstractContainerScreen and its subclasses' backgrounds, labels, slots, highlights, carried stack and hovered tooltip as GuiGraphics draw calls."""
from gui import guiparity

OWNER = "gui (Java look GUI)"


def run(h):
    guiparity.run(h, "acacia.parity.gui.ContainerOracle", "--count", 2400, probe_input="probe.jsonl")
