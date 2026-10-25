"""Item models: every item's default stack in every display context as the client submits it
(its ItemModel resolved, then ItemStackRenderState.submit: display transform, local transform,
baked quads or a special model's), against the look's Java items as acacia draws them
(`lookbake java-exported`, then item::JavaItems: the skin's quads through Display::item_frame).
Per quad: image, corners, sprite UVs and tint; probe acacia-render/examples/java_items. Then
dropped and framed items (ItemEntitiesOracle): the pose each copy is submitted under and the
model's box, against drop::java_frames, framed::java_frame and JavaItems::bounds; probe
acacia-render/examples/item_entities."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "entity_models"))
from entity_look import look_dir  # noqa: E402

OWNER = "acacia-render item (Java look items)"


def run(h):
    look = look_dir(h)
    dump = h.oracle("acacia.parity.item_models.ItemModelsOracle")
    h.read_probe(h.probe("acacia-render", "java_items", look, dump / "items.jsonl"))
    placed = h.oracle("acacia.parity.item_models.ItemEntitiesOracle")
    h.read_probe(h.probe("acacia-render", "item_entities", look, placed / "item_entities.jsonl"))
    components = h.oracle("acacia.parity.item_models.ItemComponentsOracle")
    h.read_probe(h.probe("acacia-render", "item_components", look, components / "item_components.jsonl"))
