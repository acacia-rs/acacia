"""Name tags: where each entity type's name tag hangs (the NAME_TAG attachment, adult and baby, in
every pose) and how far it shows, against acacia_render::entity::java::types; probe
acacia-render/examples/name_tags_probe."""

OWNER = "acacia-render entity (name tags)"


def run(h):
    dump = h.oracle("acacia.parity.name_tags.NameTagsOracle")
    h.read_probe(h.probe("acacia-render", "name_tags_probe", dump / "name_tags.jsonl"))
