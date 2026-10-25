"""GUI frames as GuiRenderer draws them: slices of the GUI harnesses' cases, the game's recorded calls replayed onto a real GuiGraphicsExtractor and sorted as GuiRenderer sorts them, against acacia's painter (acacia_ui::java::paint) over the same calls; item decorations from the cases' resolved stacks; picture-in-picture left out."""
import json
import os
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gui import guiparity  # noqa: E402
from paritylib import oracle  # noqa: E402
import framecompare  # noqa: E402

OWNER = "gui (Java look GUI)"

# name: (oracle main, cases, the file the oracle writes for the probe instead of the cases)
SOURCES = {
    "hud": ("HudOracle", 300, None),
    "containers": ("ContainerOracle", 200, "probe.jsonl"),
    "tooltips": ("TooltipOracle", 200, "probe.jsonl"),
    "screens_widgets": ("WidgetsOracle", 200, None),
    "screens_menus": ("MenusOracle", 200, None),
    "screens_lists": ("ListsOracle", 200, None),
    "screens_more": ("MoreOracle", 200, None),
    "screens_text": ("TextOracle", 200, None),
    "chat_screen": ("ChatScreenOracle", 200, None),
    "creative": ("CreativeOracle", 200, "probe.jsonl"),
    "recipe_book": ("RecipeBookOracle", 200, "probe.jsonl"),
    "trading": ("TradingOracle", 200, "probe.jsonl"),
    "stations": ("StationOracle", 200, "probe.jsonl"),
}


def run(h):
    subprocess.run([sys.executable, Path(guiparity.__file__).parents[1] / "harnesses" / "font" / "fetch.py"], stdout=h.log, check=True)
    only = os.environ.get("GUI_FRAME_SOURCES", "").split() or list(SOURCES)
    recorded = {}
    for name in only:
        main, count, probe_input = SOURCES[name]
        count = int(os.environ.get("GUI_FRAME_COUNT", count))
        source = h.dir.parent / name
        out = h.out / name
        out.mkdir(parents=True, exist_ok=True)
        inputs = guiparity.cases(source, out, ["--count", count])
        expected = oracle.run_oracle(source, name, f"acacia.parity.gui.{main}", [inputs], [inputs], h.fresh, h.log)
        recorded[name] = (expected / "vanilla.jsonl", expected / probe_input if probe_input else inputs)
    frames = h.oracle("acacia.parity.gui.FrameOracle", *(f"{n}={v}={p}" for n, (v, p) in recorded.items()),
                      inputs=[f for pair in recorded.values() for f in pair])
    blends = json.loads((frames / "blends.json").read_text())
    for name, (_, probe) in recorded.items():
        rust = h.out / name / "frame.jsonl"
        h.cargo("run", "--release", "-q", "-p", "acacia-gui-probe", "--", f"frame:{name}", guiparity.assets(), probe, rust)
        n, divergences = framecompare.compare(frames / f"{name}.jsonl", rust, blends, name)
        h.log.write(f"  {name:16} {n - len(divergences)}/{n}\n")
        h.add(n, divergences)
