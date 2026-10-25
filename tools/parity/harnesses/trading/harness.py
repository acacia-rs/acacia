"""List screens: MerchantScreen, StonecutterScreen and LoomScreen's lists, scroll bars, buttons, level bar and tooltips as GuiGraphics draw calls."""
from gui import guiparity

OWNER = "gui (Java look GUI)"


def run(h):
    guiparity.run(h, "acacia.parity.gui.TradingOracle", "--count", 2400, probe_input="probe.jsonl")
