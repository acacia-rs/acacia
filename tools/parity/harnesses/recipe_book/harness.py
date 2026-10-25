"""The recipe book (RecipeBookComponent: search box, tabs, filter, page of recipe buttons, overlay, ghost recipe, tooltips) alone and beside the crafting, furnace, blast furnace, smoker and inventory screens, as GuiGraphics draw calls."""
from gui import guiparity

OWNER = "gui (Java look GUI)"


def run(h):
    guiparity.run(h, "acacia.parity.gui.RecipeBookOracle", "--count", 2500, probe_input="probe.jsonl")
