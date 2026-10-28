"""Entity gear: what the render layers of the kinds that wear saddles, body armour, carpets,
harnesses, collars and markings submit over random gear (SimpleEquipmentLayer, WolfArmorLayer and
its cracks, LlamaDecorLayer, CatCollarLayer, WolfCollarLayer, HorseMarkingLayer, RopesLayer): each
model's layer, texture, colour and render type in order, against acacia_render::entity::java::gear;
probe acacia-render/examples/gear_probe."""
import sys
from pathlib import Path

sys.path[:0] = [str(Path(__file__).resolve().parents[1] / "entity_models")]
from entity_look import look_dir  # noqa: E402

OWNER = "acacia-render entity (Java look entities)"


def run(h):
    look = look_dir(h)
    dump = h.oracle("acacia.parity.entity_gear.EntityGearOracle")
    h.read_probe(h.probe("acacia-render", "gear_probe", look, dump / "gear.jsonl"))
