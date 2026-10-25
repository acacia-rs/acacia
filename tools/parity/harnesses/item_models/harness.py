"""Item models: every item's default stack in every display context as the client submits it
(its ItemModel resolved, then ItemStackRenderState.submit: display transform, local transform,
baked quads or a special model's), against the look's Java items as acacia draws them
(`lookbake java-exported`, then item::JavaItems: the skin's quads through Display::item_frame).
Per quad: image, corners, sprite UVs and tint; probe acacia-render/examples/java_items."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "entity_models"))
from entity_look import look_dir  # noqa: E402

OWNER = "acacia-render item (Java look items)"


def run(h):
    look = look_dir(h)
    dump = h.oracle("acacia.parity.item_models.ItemModelsOracle")
    h.read_probe(h.probe("acacia-render", "java_items", look, dump / "items.jsonl"))
