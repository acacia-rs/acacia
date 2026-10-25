"""Subtitles: SubtitleOverlay's captions (merging, expiry, fade, direction arrows, box width) as GuiGraphics draw calls."""
from gui import guiparity

OWNER = "gui (Java look GUI)"


def run(h):
    guiparity.run(h, "acacia.parity.gui.SubtitlesOracle", "--count", 1500)
