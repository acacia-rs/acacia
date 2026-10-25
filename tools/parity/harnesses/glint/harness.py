"""Enchantment glint: the TextureTransform matrices (item, entity, armour) over clock and
glintSpeed, and which items' default stacks shimmer, against acacia_render::glint; probe
acacia-render/examples/glint_probe."""

OWNER = "acacia-render glint"


def run(h):
    dump = h.oracle("acacia.parity.glint.GlintOracle")
    h.read_probe(h.probe("acacia-render", "glint_probe", dump / "glint.jsonl"))
