"""GUI list screens: key binds (KeyBindsList), language select and pack selection (TransferableSelectionList) as GuiGraphics draw calls."""
from gui import guiparity

OWNER = "gui (Java look GUI)"


def run(h):
    guiparity.run(h, "acacia.parity.gui.ListsOracle", "--count", 2400)
