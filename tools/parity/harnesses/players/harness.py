"""Players: the player list (PlayerTabOverlay: names, teams, ping, heads, scores and hearts, header and footer, columns) as GuiGraphics draw calls."""
from gui import guiparity

OWNER = "gui (Java look GUI)"


def run(h):
    guiparity.run(h, "acacia.parity.gui.PlayersOracle", "--count", 1000)
