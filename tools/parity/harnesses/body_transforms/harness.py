"""Body transforms: the matrix LivingEntityRenderer.submit gives a model (entity scale,
setupRotations, the flip, the renderer's scale, the 1.501 drop) for the renderers with their own
rotations or scale and plain ones, over random render states, against
acacia_render::entity::java::body; probe acacia-render/examples/body_probe."""

OWNER = "acacia-render entity (body transforms)"


def run(h):
    dump = h.oracle("acacia.parity.body_transforms.BodyTransformsOracle")
    h.read_probe(h.probe("acacia-render", "body_probe", dump / "body.jsonl"))
