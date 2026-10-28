"""Chat screen: the chat input (EditBox: caret, selection, scrolling, history) and name completion (CommandSuggestions' list) as GuiGraphics draw calls."""
from gui import guiparity

OWNER = "gui (Java look GUI)"


def run(h):
    guiparity.run(h, "acacia.parity.gui.ChatScreenOracle", "--count", 3000)
