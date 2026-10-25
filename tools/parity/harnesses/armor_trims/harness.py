"""Armour trims: per equipment asset with humanoid layers, layer type, trim pattern and material,
the texture EquipmentLayerRenderer's TrimTextureKey asks for (trim overrides included) and that
texture as PalettedTextureManager makes it, against the look's choice (Equipment::trim_texture)
and lookbake's baked image; probe acacia-render/examples/armor_trims."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "entity_models"))
from entity_look import look_dir  # noqa: E402

OWNER = "acacia-render entity (Java look entities)"


def run(h):
    look = look_dir(h)
    dump = h.oracle("acacia.parity.armor_trims.ArmorTrimsOracle")
    h.read_probe(h.probe("acacia-render", "armor_trims", look, dump / "trims.jsonl"))
