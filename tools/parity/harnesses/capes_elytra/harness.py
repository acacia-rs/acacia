"""Capes and elytra over time: ClientAvatarState's cloak and bob read by
AvatarRenderer.extractCapeState, and ElytraAnimationState, ticked through random moves, against
acacia_render::entity::java::avatar; probe acacia-render/examples/capes_elytra. The cape and wing
models' poses from these values are in entity_poses."""

OWNER = "acacia-render entity (capes, elytra)"


def run(h):
    dump = h.oracle("acacia.parity.capes_elytra.CapesElytraOracle")
    h.read_probe(h.probe("acacia-render", "capes_elytra", dump / "capes_elytra.jsonl"))
