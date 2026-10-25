"""HUD: hotbar, crosshair, hearts, food, air, xp, effects, titles, item name, boss bars, jump bar, sidebar, item decorations, camera overlays and chat as GuiGraphics draw calls."""
from gui import guiparity

OWNER = "gui (Java look GUI)"


def run(h):
    guiparity.run(h, "acacia.parity.gui.HudOracle", "--count", 2200)
