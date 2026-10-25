"""Fonts: FontManager's fonts (every provider, filters, the missing glyph), Font.width and the glyph quads Font.prepareText renders, Unifont from the asset index."""
from gui import guiparity

OWNER = "gui (Java look GUI)"


def run(h):
    guiparity.run(h, "acacia.parity.gui.FontOracle")
