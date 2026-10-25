"""Entity overlays: the hurt/dying red and the creeper's white flash (OverlayTexture coordinates and
texels), and the shake of the frozen and the converting (isShaking and setupRotations' body turn),
against acacia_render::entity::java::overlay; probe acacia-render/examples/overlays_probe."""

OWNER = "acacia-render entity (overlays)"


def run(h):
    dump = h.oracle("acacia.parity.overlays.OverlaysOracle")
    h.read_probe(h.probe("acacia-render", "overlays_probe", dump / "overlays.jsonl"))
