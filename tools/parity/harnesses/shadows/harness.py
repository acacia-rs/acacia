"""Shadows: every entity type's shadow radius and strength (adult and baby) from its renderer, and
EntityRenderer.extractShadow's pieces and ShadowFeatureRenderer's vertices over random ground,
light, heights, radii and camera distances, against acacia_render::shadows fed the oracle's
per-cell facts; probe acacia-render/examples/shadows_probe."""

OWNER = "acacia-render entity (shadows)"


def run(h):
    dump = h.oracle("acacia.parity.shadows.ShadowsOracle")
    h.read_probe(h.probe("acacia-render", "shadows_probe", dump / "shadows.jsonl"))
